#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
vpid GitHub Release 发布脚本（API 直传，无 GitHub Actions）
===========================================================
用 git-credential-manager 缓存的凭据（OAuth token）调 GitHub API：
  1. 推送当前分支到 origin（若落后）
  2. 打 annotated tag（语义化版本 vX.Y.Z），先推 tag
  3. 创建/更新 GitHub Release
  4. 上传 dist/ 下全部产物，校验大小

用法：
  python tools/release_github.py --tag v1.0.0 --title "v1.0.0" --notes-file RELEASE_NOTES.md [--branch main]

凭据来源（按优先级）：
  - 环境变量 GITHUB_TOKEN
  - git credential manager（`git credential fill` 对 https://github.com）
"""
import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIST = os.path.join(REPO, "dist")
REMOTE = "origin"
API = "https://api.github.com"
OWNER_REPO = None  # 从 git remote 解析


def git(*args, **kw):
    r = subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", **kw)
    if r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} 失败: {r.stderr.strip()}")
    return r.stdout.strip()


def get_remote_slug():
    """从 origin remote 解析 owner/repo。"""
    out = git("remote", "get-url", REMOTE)
    m = re.search(r"(?:github\.com[:/])([^/]+)/([^/]+?)(?:\.git)?$", out)
    if not m:
        raise RuntimeError(f"无法从 remote 解析 GitHub owner/repo: {out}")
    return m.group(1), m.group(2).removesuffix(".git")


def get_token():
    """取 GitHub token：环境变量优先，其次 git-credential-manager。"""
    if os.environ.get("GITHUB_TOKEN"):
        return os.environ["GITHUB_TOKEN"].strip()
    # 从 git credential fill 获取（凭据管理器缓存）
    try:
        r = subprocess.run(
            ["git", "credential", "fill"],
            input="protocol=https\nhost=github.com\n\n",
            capture_output=True, text=True, encoding="utf-8", cwd=REPO, timeout=15)
        for line in r.stdout.splitlines():
            if line.startswith("password="):
                return line[len("password="):].strip()
    except Exception:
        pass
    raise RuntimeError(
        "未找到 GitHub token。请设置环境变量 GITHUB_TOKEN，或先运行 git-credential-manager 登录 GitHub")


def api(method, path, token, body=None):
    """GitHub API 请求，返回 (status, json)。"""
    url = f"{API}{path}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"token {token}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("User-Agent", "vpid-release-script")
    if data:
        req.add_header("Content-Type", "application/json; charset=utf-8")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            raw = resp.read()
            return resp.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as e:
        raw = e.read().decode("utf-8", "replace")
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"message": raw}


def push_branch(token, branch):
    """推送分支到 origin（HTTP + 认证头）。"""
    out = git("remote", "get-url", REMOTE)
    cfg_key = f"http.{out}.extraheader"
    git("-c", f"{cfg_key}=Authorization: token {token}", "push", REMOTE, branch)
    print(f"  已推送 {branch} -> {REMOTE}")


def push_tag(token, tag, msg):
    git("tag", "-a", tag, "-m", msg)
    out = git("remote", "get-url", REMOTE)
    cfg_key = f"http.{out}.extraheader"
    git("-c", f"{cfg_key}=Authorization: token {token}", "push", REMOTE, tag)
    # 两个 SHA：tag 对象 SHA 与指向的 commit SHA
    tag_sha = git("rev-parse", tag)
    commit_sha = git("show", "--no-patch", "--format=%H", tag)
    print(f"  已推送 tag {tag}（对象 {tag_sha[:10]} -> commit {commit_sha[:10]}）")
    return tag_sha, commit_sha


def upload_asset(token, owner, repo, release_id, path):
    """上传资产，自动处理新/旧版 Gitea 不同端点（这里是 GitHub，仅 pre-signed 之外直传情况）。"""
    name = os.path.basename(path)
    from urllib.parse import quote
    url = f"{API}/repos/{owner}/{repo}/releases/{release_id}/assets?name={quote(name)}"
    with open(path, "rb") as f:
        data = f.read()
    req = urllib.request.Request(url, data=data, method="POST")
    req.add_header("Authorization", f"token {token}")
    req.add_header("Content-Type", "application/octet-stream")
    req.add_header("Accept", "application/vnd.github+json")
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            asset = json.loads(resp.read())
            print(f"  [OK] {name} ({len(data)} bytes) -> {asset.get('browser_download_url', '')}")
            return asset.get("size") == len(data)
    except urllib.error.HTTPError as e:
        print(f"  [FAIL] {name}: HTTP {e.code} {e.read().decode('utf-8', 'replace')[:200]}",
              file=sys.stderr)
        return False


def main():
    ap = argparse.ArgumentParser(description="vpid GitHub Release 发布")
    ap.add_argument("--tag", required=True, help="版本 tag，如 v1.0.0")
    ap.add_argument("--title", required=True, help="Release 标题")
    ap.add_argument("--notes-file", help="Release 说明文件路径")
    ap.add_argument("--branch", default="main", help="要推送的分支（默认 main）")
    ap.add_argument("--skip-push-branch", action="store_true", help="跳过推送分支")
    ap.add_argument("--assets-dir", default=DIST, help="产物目录（默认 dist/）")
    args = ap.parse_args()

    if not re.match(r"^v\d+\.\d+\.\d+$", args.tag):
        sys.exit("tag 格式应为 vX.Y.Z（语义化版本），如 v1.0.0")

    token = get_token()
    owner, repo = get_remote_slug()
    print(f"仓库: {owner}/{repo}  tag: {args.tag}")

    # 1. 推送分支（可选）
    if not args.skip_push_branch:
        push_branch(token, args.branch)
    else:
        print("  (跳过推送分支)")

    # 2. 推送 tag
    notes = ""
    if args.notes_file and os.path.exists(args.notes_file):
        with open(args.notes_file, "r", encoding="utf-8") as f:
            notes = f.read()
    push_tag(token, args.tag, f"{args.tag} - {args.title}")

    # 3. 创建 Release（幂等：已存在则复用）
    body = {"tag_name": args.tag, "name": args.title, "body": notes}
    st, rel = api("POST", f"/repos/{owner}/{repo}/releases", token, body)
    if st == 201:
        print(f"  Release 已创建: {rel.get('html_url')}")
    elif st in (422, 400):
        st2, rel2 = api("GET", f"/repos/{owner}/{repo}/releases/tags/{args.tag}", token)
        if st2 == 200:
            rel = rel2
            print(f"  Release 已存在，复用: {rel.get('html_url')}")
        else:
            sys.exit(f"Release 创建失败: HTTP {st} {rel}")
    else:
        sys.exit(f"Release 创建失败: HTTP {st} {rel}")

    # 4. 上传资产
    release_id = rel["id"]
    patterns = ["vpid_viewer_windows_x86.exe",
                "vpid_viewer_linux_amd64",
                "vpid_viewer_linux_arm64"]
    ok = True
    for name in patterns:
        path = os.path.join(args.assets_dir, name)
        if not os.path.exists(path):
            print(f"  [SKIP] {name}: 不存在")
            continue
        if not upload_asset(token, owner, repo, release_id, path):
            ok = False
    if not ok:
        sys.exit("部分资产上传失败")

    print("\n=== Release 完成 ===")
    print(f"  {rel.get('html_url')}")


if __name__ == "__main__":
    main()
#!/usr/bin/env bash
set -e
cat > /etc/apt/sources.list.d/debian.sources <<'EOF'
Types: deb
URIs: http://mirrors.tuna.tsinghua.edu.cn/debian
Suites: bookworm bookworm-updates
Components: main
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg

Types: deb
URIs: http://mirrors.tuna.tsinghua.edu.cn/debian-security
Suites: bookworm-security
Components: main
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
EOF
apt-get update -y >/dev/null 2>&1
apt-get install -y --no-install-recommends pkg-config libgtk-3-dev xvfb xauth >/dev/null 2>&1
echo "gtk: $(pkg-config --modversion gtk+-3.0)"
echo "rustc: $(rustc --version)"
echo "xauth: $(command -v xauth)"
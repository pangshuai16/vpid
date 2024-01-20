<script lang="ts">
  import { invoke } from "@tauri-apps/api/tauri";
  import { writeText } from "@tauri-apps/api/clipboard";
  import { exit } from "@tauri-apps/api/process";
  import { scale } from "svelte/transition";
  import Table from "./lib/table.svelte";

  let devices: DeviceInfo[] = [];
  let baseList: DeviceInfo[] = [];
  let addList: DeviceInfo[] = [];
  let removeList: DeviceInfo[] = [];
  let toastMsg = "";
  let msgTimer: number | null;

  getDevices();

  async function getDevices() {
    devices = (await invoke<RustDeviceInfo[]>("get_hid_devices"))
      .sort((a, b) => {
        if (a.vendor_id === b.vendor_id) {
          return a.product_id - b.product_id;
        } else {
          return a.vendor_id - b.vendor_id;
        }
      })
      .map(
        (item): DeviceInfo => ({
          ...item,
          vendor_id: item.vendor_id.toString(16).padStart(4, "0"),
          product_id: item.product_id.toString(16).padStart(4, "0"),
        })
      );

    if (baseList.length > 0) {
      getAddlist();
      getRemoveList();
    } else {
      saveBaseList();
    }
  }

  function getAddlist() {
    const paths = baseList.map((i) => i.path);
    addList = devices.filter((i) => !paths.includes(i.path));
  }
  function getRemoveList() {
    const paths = devices.map((i) => i.path);
    removeList = baseList.filter((i) => !paths.includes(i.path));
  }
  async function saveBaseList() {
    if (devices.length === 0) {
      await getDevices();
    } else {
      baseList = devices;
      addList = [];
      removeList = [];
      message("当前列表已设为对比基准");
    }
  }

  function clean() {
    removeList = addList = baseList = devices = [];
  }

  function copy(target: DeviceInfo | DeviceInfo[], name?: string) {
    writeText(formatText(target))
      .then(() => {
        if (Array.isArray(target)) {
          message(`${name}，复制成功`);
        } else {
          message(`${target.vendor_id}:${target.product_id} 复制成功`);
        }
      })
      .catch(() => {
        message("复制失败");
      });
  }
  function close() {
    exit();
  }

  async function message(msg: string) {
    if (msgTimer) {
      toastMsg = "";
      clearTimeout(msgTimer);
      await sleep(100);
    }
    toastMsg = msg;

    msgTimer = setTimeout(() => {
      toastMsg = "";
    }, 1600);
  }

  function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  function formatText(target: DeviceInfo | DeviceInfo[]) {
    let text = "";
    if (Array.isArray(target)) {
      const map = new Map();
      target.forEach((item) => {
        if (map.has(item.path)) return;
        map.set(item.path, item);
        text += formatText(item);
      });
    } else {
      text += `------------------------------------
${target.vendor_name} | ${target.product_name}
vid:pid 0x${target.vendor_id}:0x${target.product_id} 
------------------------------------

`;
    }
    return text;
  }
</script>

<main class="container">
  <div class="box">
    <Table data={devices} {copy} title="当前设备"></Table>
  </div>
  <div class="box">
    <Table data={addList} {copy} title="新增设备"></Table>
    <Table data={removeList} {copy} title="移除设备"></Table>
  </div>
  <div class="box">
    <button class="btn" on:click={getDevices}>刷新</button>
    <button class="btn" on:click={saveBaseList}> 设为基准</button>
    <button class="btn" on:click={() => copy(devices, "当前设备")}
      >复制当前</button
    >
    <button class="btn" on:click={clean}>清空</button>
    <button class="btn" on:click={close}>退出</button>
  </div>

  {#if toastMsg}
    <h5 class="toast" transition:scale={{ delay: 50, duration: 160 }}>
      {toastMsg}
    </h5>
  {/if}
</main>

<style>
  .container {
    height: 100vh;
    padding: 10px;
    box-sizing: border-box;
    display: flex;
    column-gap: 10px;
    justify-content: space-around;
    text-align: center;
  }
  .box {
    flex: 1;
    height: 100%;
    display: flex;
    flex-direction: column;
    row-gap: 10px;
  }
  .box:last-of-type {
    flex: none;
    width: 80px;
  }

  .toast {
    position: fixed;
    margin: auto;
    text-align: center;
    top: 6vh;
    width: 38.2vw;
    min-width: 100px;
    background-color: var(--primary);
    color: var(--primary-inverse);
    padding: 10px 20px;
    border-radius: 5px;
    box-shadow:
      1px 1px 24px var(--primary),
      1px 1px 12px var(--primary-inverse);
  }
</style>

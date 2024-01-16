<script lang="ts">
  import { invoke } from "@tauri-apps/api/tauri";
  import { writeText } from "@tauri-apps/api/clipboard";
  import { exit } from "@tauri-apps/api/process";
  import { scale } from "svelte/transition";

  interface RustDeviceInfo {
    product_name: string;
    path: string;
    vendor_id: number;
    product_id: number;
    vendor_name: string;
  }
  interface DeviceInfo {
    product_name: string;
    path: string;
    vendor_id: string;
    product_id: string;
    vendor_name: string;
  }
  let devices: DeviceInfo[] = [];
  let baseList: DeviceInfo[] = [];
  let addList: DeviceInfo[] = [];
  let removeList: DeviceInfo[] = [];
  let toastMsg = "";
  let msgTimer: number | null;

  getDevices();

  async function getDevices() {
    devices = (<RustDeviceInfo[]>await invoke("get_hid_devices"))
      .map((item) => ({
        ...item,
        product_id: item.product_id
          .toString(16)
          .padStart(4, "0")
          .toLocaleUpperCase(),
        vendor_id: item.vendor_id
          .toString(16)
          .padStart(4, "0")
          .toLocaleUpperCase(),
      }))
      .sort((a, b) => a.path.localeCompare(b.path));

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
      message("当前列表已设为新增和移除的对比基准");
    }
  }

  function clean() {
    removeList = addList = baseList = devices = [];
  }

  function copyItem(row: DeviceInfo) {
    writeText(JSON.stringify(row))
      .then(() => {
        message(`${row.vendor_id}:${row.product_id} 复制成功`);
      })
      .catch(() => {
        message("复制失败");
      });
  }
  function copyCurrentList() {
    writeText(JSON.stringify(devices))
      .then(() => {
        message("复制当前设备列表，成功！");
      })
      .catch(() => {
        message("复制当前设备列表，失败");
      });
  }
  function copy(target: DeviceInfo | DeviceInfo[], name?: string) {
    writeText(JSON.stringify(target))
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
    }, 2500);
  }

  function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
</script>

<main class="container">
  <div class="box1">
    <h3 class="lsit-title" on:click={() => copy(devices, "当前设备列表")}>
      当前设备列表
    </h3>
    <div class="list-outer">
      <table>
        <thead>
          <tr>
            <th scope="col">设备</th>
            <th scope="col">VID</th>
            <th scope="col">PID</th>
          </tr>
        </thead>
        <tbody>
          {#each devices as item}
            <tr on:click={() => copy(item)}>
              <td>{item.product_name}</td>
              <td>{item.vendor_id}</td>
              <td>{item.product_id}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
  <div class="box2">
    <h3
      class="lsit-title"
      on:click={() => {
        copy(addList, "新增的设备");
      }}
    >
      新增的设备
    </h3>
    <div class="list-outer">
      <table>
        <thead>
          <tr>
            <th scope="col">设备</th>
            <th scope="col">VID</th>
            <th scope="col">PID</th>
          </tr>
        </thead>
        <tbody>
          {#each addList as item}
            <tr on:click={() => copy(item)}>
              <td>{item.product_name}</td>
              <td>{item.vendor_id}</td>
              <td>{item.product_id}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <h3 class="lsit-title" on:click={() => copy(removeList, "移除的设备")}>
      移除的设备
    </h3>
    <div class="list-outer">
      <table>
        <thead>
          <tr>
            <th scope="col">设备</th>
            <th scope="col">VID</th>
            <th scope="col">PID</th>
          </tr>
        </thead>
        <tbody>
          {#each removeList as item}
            <tr on:click={() => copy(item)}>
              <td>{item.product_name}</td>
              <td>{item.vendor_id}</td>
              <td>{item.product_id}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>

  <div class="box3">
    <button on:click={getDevices}>刷新</button>
    <button on:click={saveBaseList}> 设为基准</button>
    <button on:click={copyCurrentList}>复制当前</button>
    <button on:click={clean}>清空</button>
    <button on:click={close}>退出</button>
  </div>

  {#if toastMsg}
    <h5 class="toast" transition:scale={{ delay: 50, duration: 500 }}>
      {toastMsg}
    </h5>
  {/if}
</main>

<style>
  .container {
    height: 100vh;
    padding: 4px;
    box-sizing: border-box;
  }
  .box1,
  .box2,
  .box3 {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .box1,
  .box2 {
    width: calc(50% - 60px);
  }
  .lsit-title {
    margin: 4px;
  }
  .list-outer {
    height: 100%;
    padding: 1vh 0;
    overflow-y: auto;
    border-radius: 8px;
    box-shadow:
      inset 0 0 12px var(--color),
      inset 0 0 24px var(--bg-color);
  }
  table {
    width: 100%;
  }

  .box3 {
    width: 80px;
  }
  button {
    margin: 8px 0;
  }
  .toast {
    position: fixed;
    margin: auto;
    text-align: center;
    top: 6vh;
    width: 38.2vw;
    min-width: 100px;
    background-color: var(--bg-color);
    color: var(--color);
    padding: 10px 20px;
    border-radius: 5px;
    box-shadow:
      1px 1px 24px var(--color),
      1px 1px 12px var(--bg-color);
  }
</style>

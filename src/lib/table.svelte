<script lang="ts">
  export let title: string;
  export let data: DeviceInfo[];
  export let copy: (target: DeviceInfo | DeviceInfo[], title?: string) => void;
</script>

<div class="table-container">
  <a
    href=";"
    class="title"
    on:click|preventDefault={() => {
      copy(data, title);
    }}
  >
    {title}
  </a>
  <div class="table-outer">
    <table>
      <thead>
        <tr>
          <th scope="col">设备</th>
          <th scope="col">VID</th>
          <th scope="col">PID</th>
        </tr>
      </thead>
      <tbody>
        {#each data as item}
          <tr on:click|preventDefault={() => copy(item)}>
            <td>{item.product_name}</td>
            <td>{item.vendor_id}</td>
            <td>{item.product_id}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .table-container {
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
  }
  .title {
    display: block;
    margin: 12px;
    color: var(--primary);
    font-size: 18px;
    font-weight: 500;
    text-decoration: none;
  }
  .table-outer {
    flex: 1;
    padding: 10px;
    border-radius: 8px;
    overflow-y: auto;
    box-shadow:
      inset 0 0 12px #333,
      inset 0 0 24px #eee;
  }

  tbody tr:hover td {
    color: var(--primary-inverse);
    background-color: var(--primary);
    transition:
      background-color 0.15s,
      color 0.125s;
  }
</style>

<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { attachConsole, info, error } from "@tauri-apps/plugin-log";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  type CursorMode = "software" | "hardware";
  type Orientation = "default" | "rotate-left" | "rotate-right" | "upside-down";

  type MonitorInfoDto = {
    path: string;
    name: string;
    orientation: Orientation;
    refresh_rate: number;
  };

  type InspectorState = {
    running: boolean;
    current_monitor: MonitorInfoDto|null;
    cursor_mode: CursorMode;
  };

  let detachConsole: any | undefined;
  let unlisten: UnlistenFn | undefined;

  let loading = true;
  let pending = false;
  let errMsg = "";

  let state: InspectorState | null = null;

  const orientationLabel: Record<Orientation, string> = {
    default: "0° Landscape",
    "rotate-left": "90° Left",
    "rotate-right": "90° Right",
    "upside-down": "180°",
  };

  async function refreshState() {
    state = await invoke<InspectorState>("get_inspector_state");
  }

  async function startInspector() {
    if (pending) return;
    pending = true;
    errMsg = "";
    try {
      await invoke("start_inspector");
      await refreshState();
    } catch (e) {
      errMsg = String(e);
      error(`start_inspector failed: ${errMsg}`);
    } finally {
      pending = false;
    }
  }

  async function stopInspector() {
    if (pending) return;
    pending = true;
    errMsg = "";
    try {
      await invoke("stop_inspector");
      await refreshState();
    } catch (e) {
      errMsg = String(e);
      error(`stop_inspector failed: ${errMsg}`);
    } finally {
      pending = false;
    }
  }

  async function setMode(mode: CursorMode) {
    if (pending) return;
    pending = true;
    errMsg = "";
    try {
      state = await invoke<InspectorState>("toggle_mouse_mode", { mode });
    } catch (e) {
      errMsg = String(e);
      error(`toggle_mouse_mode failed: ${errMsg}`);
    } finally {
      pending = false;
    }
  }

  onMount(async () => {
    try {
      detachConsole = await attachConsole();
      info("UI mounted");

      unlisten = await listen<InspectorState>("inspector-update", (event) => {
        state = event.payload;
      });

      await refreshState();

      requestAnimationFrame(() =>
        requestAnimationFrame(async () => {
          await tick();
          await tick();
          const window = getCurrentWindow();
          await window.show();
          await window.setFocus();
        }),
      );
    } catch (e) {
      errMsg = String(e);
      error(`startup failed: ${errMsg}`);
    } finally {
      loading = false;
    }
  });

  onDestroy(async () => {
    if (unlisten) unlisten();
    if (detachConsole) await detachConsole();
  });
</script>

<main class="page">
  <section class="panel">
    <header class="panel-head">
      <h1>Portrait Monitor Fixer</h1>
      <p>Live monitor tracking and mode control</p>
    </header>

    {#if loading}
      <div class="card muted" transition:fade={{ duration: 200 }}>
        Loading state...
      </div>
    {:else if !state}
      <div class="card error" transition:fade={{ duration: 200 }}>
        No state available. {errMsg}
      </div>
    {:else}
      <div class="grid" transition:fade={{ duration: 200 }}>
        <article class="card">
          <h2>Status</h2>
          <div class="row">
            <span>Inspector</span>
            {#key state.running}
              <strong
                class:ok={state.running}
                class:bad={!state.running}
              >
                {state.running ? "Running" : "Stopped"}
              </strong>
            {/key}
          </div>
          <div class="row">
            <span>Cursor Mode</span>
            {#key state.cursor_mode}
              <strong
                class="mode-badge"
                class:mode-software={state.cursor_mode === "software"}
                class:mode-hardware={state.cursor_mode === "hardware"}
                in:fly={{ y: -8, duration: 250 }}
              >
                {state.cursor_mode.toUpperCase()}
              </strong>
            {/key}
          </div>
        </article>

        <article class="card">
          <h2>Current Monitor</h2>
          <div class="row">
            <span>Name</span>
            {#key state.current_monitor?.name}
              <strong in:fly={{ x: 12, duration: 300 }}>
                {state.current_monitor?.name ?? "-"}
              </strong>
            {/key}
          </div>
          <div class="row">
            <span>Orientation</span>
            {#key state.current_monitor?.orientation}
              <strong in:fly={{ x: 12, duration: 300 }}>
                {orientationLabel[state.current_monitor?.orientation || "default"]}
              </strong>
            {/key}
          </div>
          <div class="row">
            <span>Refresh</span>
            {#key state.current_monitor?.refresh_rate}
              <strong in:fly={{ x: 12, duration: 300 }}>
                {state.current_monitor?.refresh_rate.toFixed(1)} Hz
              </strong>
            {/key}
          </div>
          {#key state.current_monitor?.path}
            <div class="path" in:fade={{ duration: 300 }}>
              {state.current_monitor?.path}
            </div>
          {/key}
        </article>
      </div>

      <section class="controls">
        <button
          class="btn"
          on:click={startInspector}
          disabled={pending || state.running}
        >
          Start
        </button>
        <button
          class="btn"
          on:click={stopInspector}
          disabled={pending || !state.running}
        >
          Stop
        </button>
        <button
          class="btn accent"
          on:click={() => setMode("software")}
          disabled={pending || state.cursor_mode === "software"}
        >
          Force Software
        </button>
        <button
          class="btn accent"
          on:click={() => setMode("hardware")}
          disabled={pending || state.cursor_mode === "hardware"}
        >
          Force Hardware
        </button>
      </section>
    {/if}

    {#if errMsg}
      <footer class="card error" transition:fly={{ y: 8, duration: 250 }}>
        {errMsg}
      </footer>
    {/if}
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
    font-family: "Avenir Next", "Segoe UI", sans-serif;
    background: radial-gradient(
      circle at 20% 20%,
      #f6fbff 0%,
      #ecf4ff 40%,
      #e9ffe9 100%
    );
    color: #172026;
  }

  .page {
    height: 100vh;
    overflow: hidden;
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .panel {
    width: min(920px, 100%);
    background: rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(8px);
    border: 1px solid #d6e2ee;
    border-radius: 16px;
    padding: 20px;
    box-shadow: 0 14px 40px rgba(12, 38, 67, 0.12);
  }

  .panel-head h1 {
    margin: 0;
    font-size: 28px;
    letter-spacing: 0.02em;
  }

  .panel-head p {
    margin: 6px 0 0;
    color: #4a5b6a;
  }

  .grid {
    margin-top: 16px;
    display: grid;
    gap: 14px;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  }

  .card {
    background: #ffffff;
    border: 1px solid #e3ebf3;
    border-radius: 12px;
    padding: 14px;
  }

  .card h2 {
    margin: 0 0 10px;
    font-size: 15px;
    color: #314a61;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin: 8px 0;
    font-size: 14px;
    /* prevent layout shift when text swaps */
    min-height: 24px;
  }

  .path {
    margin-top: 8px;
    font-size: 12px;
    color: #5f7388;
    word-break: break-all;
    background: #f4f8fc;
    border-radius: 8px;
    padding: 8px;
  }

  /* cursor mode badge with color transition */
  .mode-badge {
    padding: 3px 10px;
    border-radius: 20px;
    font-size: 13px;
    transition:
      background 0.35s ease,
      color 0.35s ease;
  }

  .mode-software {
    background: #fff3cd;
    color: #7a5c00;
  }

  .mode-hardware {
    background: #d4edda;
    color: #155724;
  }

  /* running/stopped color transition */
  .ok {
    color: #0d8a4f;
    transition: color 0.3s ease;
  }

  .bad {
    color: #c03535;
    transition: color 0.3s ease;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 16px;
  }

  .btn {
    border: 0;
    border-radius: 10px;
    padding: 10px 14px;
    background: #203a55;
    color: #fff;
    font-weight: 600;
    cursor: pointer;
    transition:
      transform 0.12s ease,
      opacity 0.12s ease,
      background 0.2s ease;
  }

  .btn.accent {
    background: #0e8f6b;
  }

  .btn:hover:enabled {
    transform: translateY(-1px);
  }

  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .muted {
    color: #5f7388;
  }

  .error {
    border-color: #ffd0d0;
    background: #fff6f6;
    color: #a32727;
    margin-top: 12px;
  }
</style>

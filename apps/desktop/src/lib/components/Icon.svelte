<script lang="ts">
  /** Single stroke icon set (16×16 grid, 1.5 stroke) shared by the whole shell. */
  const paths: Record<string, string> = {
    transcribe: "M2 8.6h1.9l1.4-4 2 7.2 1.6-4.6 1.3 2.4h1.8",
    history: "M2.4 8a5.6 5.6 0 1 0 11.2 0a5.6 5.6 0 1 0-11.2 0 M8 4.9V8l2.1 1.3",
    model:
      "M3.6 3.6h8.8v8.8H3.6z M6.1 1.9v1.7M9.9 1.9v1.7M6.1 12.4v1.7M9.9 12.4v1.7M1.9 6.1h1.7M1.9 9.9h1.7M12.4 6.1h1.7M12.4 9.9h1.7",
    settings:
      "M2.4 5.4h1.6M7.7 5.4h5.9M2.4 10.6h5.9M11.9 10.6h1.7 M4.4 5.4a1.7 1.7 0 1 0 3.4 0a1.7 1.7 0 1 0-3.4 0 M8.6 10.6a1.7 1.7 0 1 0 3.4 0a1.7 1.7 0 1 0-3.4 0",
    panel: "M2.3 3.2h11.4v9.6H2.3z M6.4 3.2v9.6",
    sidebar:
      "M2.5 3.5h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1Z M6.2 3.5v9 M3.6 6.4h1.1 M3.6 9h1.1",
    home: "M2.7 2.7h4.4v4.4H2.7z M8.9 2.7h4.4v4.4H8.9z M2.7 8.9h4.4v4.4H2.7z M8.9 8.9h4.4v4.4H8.9z",
    queue:
      "M6.45 4.6a.95 .95 0 1 0 1.9 0a.95 .95 0 1 0-1.9 0 M6.45 8a.95 .95 0 1 0 1.9 0a.95 .95 0 1 0-1.9 0 M6.45 11.4a.95 .95 0 1 0 1.9 0a.95 .95 0 1 0-1.9 0 M9.6 4.6h3.6M9.6 8h3.6M9.6 11.4h3.6",
    file: "M5 2.5h4.2L12.5 5.8V13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1Z M9 2.6v3.4h3.4",
    copy: "M5.2 5.2h8v8h-8z M10.8 5.2V3.8a1.6 1.6 0 0 0-1.6-1.6H3.8a1.6 1.6 0 0 0-1.6 1.6v5.4a1.6 1.6 0 0 0 1.6 1.6h1.4",
    download: "M8 2.8v7.4M5.2 7.6 8 10.4l2.8-2.8M3.2 12.4h9.6",
    search: "M3.3 7.2a3.9 3.9 0 1 0 7.8 0a3.9 3.9 0 1 0-7.8 0 M10.2 10.2 13 13",
    chevronDown: "M3 4.5 6 7.5 9 4.5",
    chevronLeft: "M10 4 6 8l4 4",
    chevronRight: "M6 4l4 4-4 4",
    grip: "M4 5h8M4 8h8M4 11h8",
    plus: "M8 3.4v9.2M3.4 8h9.2",
    info: "M2.4 8a5.6 5.6 0 1 0 11.2 0a5.6 5.6 0 1 0-11.2 0 M8 7.4v3.4M8 5.3v.1",
    warn: "M8 2.6 14 13H2L8 2.6Z M8 6.6v3M8 11.2v.1",
    arrowDown: "M8 4v8M4.6 8.6 8 12l3.4-3.4",
    sun: "M8 6.2a1.8 1.8 0 1 0 0 3.6a1.8 1.8 0 1 0 0-3.6 M8 1.9v1.3M8 12.8v1.3M1.9 8h1.3M12.8 8h1.3M3.7 3.7l.9.9M11.4 11.4l.9.9M12.3 3.7l-.9.9M4.6 11.4l-.9.9",
    moon: "M9.6 2.6a5.4 5.4 0 1 0 3.8 9.1A5.6 5.6 0 0 1 9.6 2.6Z",
  };

  let {
    name,
    size = 16,
    stroke = 1.5,
  }: { name: keyof typeof paths | string; size?: number; stroke?: number } = $props();
</script>

<svg
  viewBox="0 0 16 16"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <path d={paths[name] ?? ""} />
</svg>

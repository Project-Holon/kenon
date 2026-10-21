<script lang="ts">
  import { onMount } from "svelte";
  import { loadMonaco, type Marker } from "./monaco";

  // A text editor (Monaco) for code: syntax colours, completion, and
  // problems drawn on the lines. It follows the day and night themes.
  // Monaco loads when the first editor opens.
  let {
    value = $bindable(""),
    language = "plaintext",
    markers = [],
    readOnly = false,
    onSave,
    label = "Editor",
  }: {
    value?: string;
    language?: string;
    markers?: Marker[];
    readOnly?: boolean;
    /// Called on Ctrl+S inside the editor.
    onSave?: () => void;
    label?: string;
  } = $props();

  type Monaco = typeof import("./monacoSetup").monaco;

  let host = $state<HTMLDivElement | null>(null);
  let mon = $state.raw<Monaco | null>(null);
  let editor: ReturnType<Monaco["editor"]["create"]> | undefined;
  let model: ReturnType<Monaco["editor"]["createModel"]> | undefined;
  // Keeps the editor and `value` from echoing into each other.
  let own = false;

  onMount(() => {
    let gone = false;
    loadMonaco().then(({ monaco, ensureMonaco }) => {
      if (gone || !host) return;
      ensureMonaco();
      model = monaco.editor.createModel(value, language);
      editor = monaco.editor.create(host, {
        model,
        automaticLayout: true,
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        wordWrap: "on",
        fontSize: 13,
        fontFamily: "ui-monospace, Consolas, monospace",
        tabSize: 4,
        insertSpaces: true,
        renderWhitespace: "selection",
        readOnly,
        ariaLabel: label,
        fixedOverflowWidgets: true,
        padding: { top: 6, bottom: 6 },
      });
      editor.onDidChangeModelContent(() => {
        if (own) return;
        own = true;
        value = model!.getValue();
        own = false;
      });
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => onSave?.());
      mon = monaco;
    });
    return () => {
      gone = true;
      editor?.dispose();
      model?.dispose();
    };
  });

  $effect(() => {
    const next = value;
    if (!mon || !model || own || next === model.getValue()) return;
    own = true;
    model.setValue(next);
    own = false;
  });

  $effect(() => {
    if (mon && model) mon.editor.setModelLanguage(model, language);
  });

  $effect(() => {
    editor?.updateOptions({ readOnly });
  });

  $effect(() => {
    if (!mon || !model) return;
    const text = model;
    mon.editor.setModelMarkers(
      text,
      "kenon",
      markers
        .filter((m) => m.line >= 1 && m.line <= text.getLineCount())
        .map((m) => ({
          startLineNumber: m.line,
          endLineNumber: m.line,
          startColumn: m.start,
          endColumn: m.end,
          message: m.message,
          severity: m.severity === "error" ? mon!.MarkerSeverity.Error : mon!.MarkerSeverity.Warning,
        })),
    );
  });
</script>

<div class="code" bind:this={host}></div>

<style>
  .code {
    width: 100%;
    height: 100%;
    min-height: 8rem;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
  }
</style>

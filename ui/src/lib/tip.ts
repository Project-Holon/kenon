// A tooltip that always stays on screen: shown under (or above) its
// element on hover or focus, moved left or up when it would leave the
// window. Use: <span use:tip={"text"}>.
import type { Action } from "svelte/action";

const MARGIN = 8;

export const tip: Action<HTMLElement, string> = (node, initial) => {
  let text = initial;
  let box: HTMLDivElement | null = null;

  function show() {
    if (!text || box) return;
    box = document.createElement("div");
    box.className = "tip-box";
    box.textContent = text;
    document.body.appendChild(box);
    const r = node.getBoundingClientRect();
    const b = box.getBoundingClientRect();
    let left = r.left;
    let top = r.bottom + 6;
    if (left + b.width > window.innerWidth - MARGIN) left = window.innerWidth - MARGIN - b.width;
    if (top + b.height > window.innerHeight - MARGIN) top = r.top - 6 - b.height;
    box.style.left = `${Math.max(MARGIN, left)}px`;
    box.style.top = `${Math.max(MARGIN, top)}px`;
  }
  function hide() {
    box?.remove();
    box = null;
  }
  node.addEventListener("mouseenter", show);
  node.addEventListener("mouseleave", hide);
  node.addEventListener("focus", show);
  node.addEventListener("blur", hide);
  return {
    update(next) {
      text = next;
      hide();
    },
    destroy() {
      hide();
      node.removeEventListener("mouseenter", show);
      node.removeEventListener("mouseleave", hide);
      node.removeEventListener("focus", show);
      node.removeEventListener("blur", hide);
    },
  };
};

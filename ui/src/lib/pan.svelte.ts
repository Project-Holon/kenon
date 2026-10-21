// Middle-button dragging: moves a canvas inside its stage by an offset,
// whether or not the canvas is larger than the stage.
export class Pan {
  x = $state(0);
  y = $state(0);
  private from: { px: number; py: number; x: number; y: number } | null = null;

  get style(): string {
    return `transform: translate(${this.x}px, ${this.y}px)`;
  }

  reset() {
    this.x = 0;
    this.y = 0;
  }

  // Use as the stage's onpointerdowncapture: a middle press starts a pan
  // and never reaches the canvas.
  down = (e: PointerEvent) => {
    if (e.button !== 1) return;
    e.preventDefault();
    e.stopPropagation();
    this.from = { px: e.clientX, py: e.clientY, x: this.x, y: this.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  move = (e: PointerEvent) => {
    if (!this.from) return;
    e.stopPropagation();
    this.x = this.from.x + e.clientX - this.from.px;
    this.y = this.from.y + e.clientY - this.from.py;
  };

  up = (e: PointerEvent) => {
    if (!this.from || e.button !== 1) return;
    e.stopPropagation();
    this.from = null;
  };

  // Use as onpointercancel / onlostpointercapture: ends a pan that lost its pointer.
  cancel = () => {
    this.from = null;
  };
}

//! NoteLume-style right-click menu (`.cv-menu` parity): one singleton menu,
//! fixed positioning clamped to the viewport, hover submenus, and the same
//! dismiss rules (outside mousedown, Esc, wheel outside, resize). Visual tokens
//! live in `styles.css` under `.ctx-menu`; this module owns only behaviour.

export interface CtxMenuItem {
  label: string;
  hint?: string;
  icon?: string;
  disabled?: boolean;
  danger?: boolean;
  checked?: boolean;
  run: () => void;
}

export interface CtxSubMenu {
  label: string;
  icon?: string;
  disabled?: boolean;
  items: CtxRow[];
}

export type CtxRow = CtxMenuItem | CtxSubMenu | "sep";

export function isSubMenu(row: CtxRow): row is CtxSubMenu {
  return typeof row === "object" && row !== null && "items" in row;
}

/** Keep a menu fully on screen with a 4px gutter (NoteLume `placeMenu`). */
export function clampMenuPosition(
  x: number,
  y: number,
  width: number,
  height: number,
  viewWidth: number,
  viewHeight: number,
): { left: number; top: number } {
  const margin = 4;
  return {
    left: Math.max(margin, Math.min(x, viewWidth - width - margin)),
    top: Math.max(margin, Math.min(y, viewHeight - height - margin)),
  };
}

function svgIcon(id: string): SVGSVGElement {
  const NS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("class", "ico");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(NS, "use");
  use.setAttribute("href", "#" + id);
  svg.append(use);
  return svg;
}

let panels: HTMLElement[] = [];
let cleanup: (() => void) | null = null;

export function closeContextMenu(): void {
  cleanup?.();
  cleanup = null;
  for (const p of panels) p.remove();
  panels = [];
}

export function isContextMenuOpen(): boolean {
  return panels.length > 0;
}

function itemNodes(row: CtxMenuItem): HTMLButtonElement {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "ctx-menu-item" + (row.danger ? " danger" : "");
  b.tabIndex = -1;
  if (row.disabled) b.disabled = true;

  const chk = document.createElement("span");
  chk.className = "ctx-menu-check";
  if (row.checked) chk.append(svgIcon("i-check"));
  b.append(chk);

  if (row.icon) {
    const ic = svgIcon(row.icon);
    ic.classList.add("ctx-menu-ico");
    b.append(ic);
  }

  const label = document.createElement("span");
  label.className = "ctx-menu-label";
  label.textContent = row.label;
  b.append(label);

  if (row.hint) {
    const hint = document.createElement("span");
    hint.className = "ctx-menu-hint";
    hint.textContent = row.hint;
    b.append(hint);
  }

  b.addEventListener("mousedown", (e) => e.preventDefault());
  b.addEventListener("click", () => {
    if (b.disabled) return;
    closeContextMenu();
    row.run();
  });
  return b;
}

function placeSub(parent: HTMLElement, sub: HTMLElement, owner: HTMLElement): void {
  document.body.append(sub);
  panels.push(sub);
  const r = owner.getBoundingClientRect();
  const sw = sub.offsetWidth;
  const sh = sub.offsetHeight;
  let left = r.right + 2;
  if (left + sw > window.innerWidth - 4) left = r.left - sw - 2;
  left = Math.max(4, left);
  const top = Math.max(4, Math.min(r.top - 4, window.innerHeight - sh - 4));
  sub.style.left = `${left}px`;
  sub.style.top = `${top}px`;
  void parent;
}

function subMenuNode(row: CtxSubMenu): HTMLButtonElement {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "ctx-menu-item ctx-menu-parent";
  b.tabIndex = -1;
  if (row.disabled) b.disabled = true;

  const chk = document.createElement("span");
  chk.className = "ctx-menu-check";
  b.append(chk);

  if (row.icon) {
    const ic = svgIcon(row.icon);
    ic.classList.add("ctx-menu-ico");
    b.append(ic);
  }

  const label = document.createElement("span");
  label.className = "ctx-menu-label";
  label.textContent = row.label;
  b.append(label);

  const arrow = document.createElement("span");
  arrow.className = "ctx-menu-arrow";
  arrow.textContent = "›";
  b.append(arrow);

  let openSub: HTMLElement | null = null;
  let closeTimer = 0;
  const open = (): void => {
    if (b.disabled) return;
    window.clearTimeout(closeTimer);
    if (openSub) return;
    const sub = buildMenu(row.items, false);
    openSub = sub;
    placeSub(b, sub, b);
    sub.addEventListener("mouseenter", () => window.clearTimeout(closeTimer));
    sub.addEventListener("mouseleave", () => {
      closeTimer = window.setTimeout(close, 160);
    });
  };
  const close = (): void => {
    window.clearTimeout(closeTimer);
    if (!openSub) return;
    const el = openSub;
    openSub = null;
    panels = panels.filter((p) => p !== el);
    el.remove();
  };

  b.addEventListener("mouseenter", open);
  b.addEventListener("mouseleave", () => {
    closeTimer = window.setTimeout(close, 160);
  });
  b.addEventListener("mousedown", (e) => e.preventDefault());
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    if (!b.disabled) open();
  });
  return b;
}

function buildMenu(rows: readonly CtxRow[], topLevel: boolean): HTMLElement {
  const menu = document.createElement("div");
  menu.className = topLevel ? "ctx-menu" : "ctx-menu ctx-menu-sub";
  menu.setAttribute("role", "menu");
  for (const row of rows) {
    if (row === "sep") {
      const sep = document.createElement("div");
      sep.className = "ctx-menu-sep";
      sep.setAttribute("role", "separator");
      menu.append(sep);
    } else if (isSubMenu(row)) {
      menu.append(subMenuNode(row));
    } else {
      menu.append(itemNodes(row));
    }
  }
  menu.addEventListener("mousedown", (e) => e.preventDefault());
  return menu;
}

export function showContextMenu(x: number, y: number, rows: readonly CtxRow[]): void {
  closeContextMenu();
  const menu = buildMenu(rows, true);
  document.body.append(menu);
  panels = [menu];

  const pos = clampMenuPosition(
    x,
    y,
    menu.offsetWidth,
    menu.offsetHeight,
    window.innerWidth,
    window.innerHeight,
  );
  menu.style.left = `${pos.left}px`;
  menu.style.top = `${pos.top}px`;

  const inside = (node: EventTarget | null): boolean =>
    node instanceof Node && panels.some((p) => p.contains(node));

  const onDown = (e: MouseEvent): void => {
    if (!inside(e.target)) closeContextMenu();
  };
  const onWheel = (e: WheelEvent): void => {
    if (!inside(e.target)) closeContextMenu();
  };
  const onResize = (): void => closeContextMenu();
  const onKey = (e: KeyboardEvent): void => {
    const items = Array.from(
      menu.querySelectorAll<HTMLButtonElement>(".ctx-menu-item:not(:disabled)"),
    );
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeContextMenu();
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
      if (items.length === 0) return;
      e.preventDefault();
      const at = items.indexOf(document.activeElement as HTMLButtonElement);
      let next: number;
      if (e.key === "Home") next = 0;
      else if (e.key === "End") next = items.length - 1;
      else if (e.key === "ArrowDown") next = at < 0 ? 0 : (at + 1) % items.length;
      else next = at <= 0 ? items.length - 1 : at - 1;
      items[next]?.focus();
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      const focused = document.activeElement as HTMLButtonElement | null;
      if (focused && menu.contains(focused) && !focused.disabled) {
        e.preventDefault();
        focused.click();
      }
    }
  };

  document.addEventListener("mousedown", onDown, true);
  window.addEventListener("wheel", onWheel, true);
  window.addEventListener("resize", onResize);
  window.addEventListener("keydown", onKey, true);
  cleanup = () => {
    document.removeEventListener("mousedown", onDown, true);
    window.removeEventListener("wheel", onWheel, true);
    window.removeEventListener("resize", onResize);
    window.removeEventListener("keydown", onKey, true);
  };
}
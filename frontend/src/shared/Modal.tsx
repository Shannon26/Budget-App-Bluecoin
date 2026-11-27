import {
  useLayoutEffect,
  useRef,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

interface ModalRecord {
  id: symbol;
  dialog: HTMLElement;
  returnFocus: HTMLElement | null;
}

const modalStack: ModalRecord[] = [];
const focusableSelector = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

let backgroundState: {
  shell: HTMLElement;
  inert: boolean;
  ariaHidden: string | null;
  bodyOverflow: string;
} | null = null;

function isVisible(element: HTMLElement): boolean {
  return (
    element.isConnected &&
    element.getClientRects().length > 0 &&
    element.getAttribute("aria-hidden") !== "true" &&
    element.closest("[inert]") === null
  );
}

function focusableElements(dialog: HTMLElement): HTMLElement[] {
  return [...dialog.querySelectorAll<HTMLElement>(focusableSelector)].filter(isVisible);
}

function preferredFocus(dialog: HTMLElement): HTMLElement {
  const explicit = dialog.querySelector<HTMLElement>("[data-autofocus]");
  if (explicit && isVisible(explicit)) return explicit;
  const firstField = focusableElements(dialog).find((element) =>
    ["INPUT", "SELECT", "TEXTAREA"].includes(element.tagName),
  );
  if (firstField) return firstField;
  return focusableElements(dialog)[0] ?? dialog;
}

function updateModalState() {
  const top = modalStack.at(-1);
  for (const record of modalStack) {
    const active = record === top;
    if (active) {
      record.dialog.removeAttribute("inert");
      record.dialog.removeAttribute("aria-hidden");
    } else {
      record.dialog.setAttribute("inert", "");
      record.dialog.setAttribute("aria-hidden", "true");
    }
  }

  const shell = document.querySelector<HTMLElement>(".app-shell");
  if (modalStack.length > 0 && shell) {
    if (!backgroundState) {
      backgroundState = {
        shell,
        inert: shell.hasAttribute("inert"),
        ariaHidden: shell.getAttribute("aria-hidden"),
        bodyOverflow: document.body.style.overflow,
      };
    }
    shell.setAttribute("inert", "");
    shell.setAttribute("aria-hidden", "true");
    document.body.style.overflow = "hidden";
  } else if (backgroundState) {
    const previous = backgroundState;
    if (previous.inert) previous.shell.setAttribute("inert", "");
    else previous.shell.removeAttribute("inert");
    if (previous.ariaHidden === null) previous.shell.removeAttribute("aria-hidden");
    else previous.shell.setAttribute("aria-hidden", previous.ariaHidden);
    document.body.style.overflow = previous.bodyOverflow;
    backgroundState = null;
  }
}

function restoreFocus(target: HTMLElement | null) {
  if (target && isVisible(target)) {
    target.focus();
    return;
  }
  const activeNavigation = document.querySelector<HTMLElement>(
    '.main-nav button[aria-current="page"]',
  );
  if (activeNavigation && isVisible(activeNavigation)) {
    activeNavigation.focus();
    return;
  }
  const heading = document.querySelector<HTMLElement>("main h1, main h2");
  if (heading) {
    heading.tabIndex = -1;
    heading.focus();
  }
}

export default function Modal({
  children,
  label,
  className = "",
  onClose,
  dismissible = true,
}: {
  children: ReactNode;
  label: string;
  className?: string;
  onClose: () => void;
  dismissible?: boolean;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  const closeRef = useRef(onClose);
  const dismissibleRef = useRef(dismissible);
  closeRef.current = onClose;
  dismissibleRef.current = dismissible;

  useLayoutEffect(() => {
    const dialog = dialogRef.current!;
    const record: ModalRecord = {
      id: Symbol("modal"),
      dialog,
      returnFocus: document.activeElement instanceof HTMLElement ? document.activeElement : null,
    };
    modalStack.push(record);
    updateModalState();
    preferredFocus(dialog).focus();

    function isActive() {
      return modalStack.at(-1) === record;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (!isActive()) return;
      if (event.key === "Escape" && dismissibleRef.current) {
        event.preventDefault();
        event.stopPropagation();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = focusableElements(dialog);
      if (focusable.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || active === dialog || !dialog.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
        event.preventDefault();
        first.focus();
      }
    }

    function handleFocusIn(event: FocusEvent) {
      if (!isActive()) return;
      if (!dialog.contains(event.target as Node)) {
        event.preventDefault();
        preferredFocus(dialog).focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown, true);
    document.addEventListener("focusin", handleFocusIn, true);
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      document.removeEventListener("focusin", handleFocusIn, true);
      const wasActive = modalStack.at(-1) === record;
      const index = modalStack.indexOf(record);
      if (index >= 0) modalStack.splice(index, 1);
      updateModalState();
      if (wasActive) restoreFocus(record.returnFocus);
    };
  }, []);

  return createPortal(
    <div className="modal-backdrop" role="presentation">
      <section
        ref={dialogRef}
        className={`modal-card ${className}`.trim()}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
      >
        {children}
      </section>
    </div>,
    document.body,
  );
}

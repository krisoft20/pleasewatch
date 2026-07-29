export function clickOutside(node: HTMLElement, close: () => void) {
    function onDoc(ev: MouseEvent) {
        if (!node.contains(ev.target as Node)) close();
    }
    function onKey(ev: KeyboardEvent) {
        if (ev.key === 'Escape') close();
    }
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return {
        destroy() {
            document.removeEventListener('mousedown', onDoc);
            document.removeEventListener('keydown', onKey);
        }
    };
}

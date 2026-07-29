export type Category = 'video' | 'manga' | 'books';

export const CATEGORIES: Category[] = ['video', 'manga', 'books'];

const LS_KEY = 'pw-category';

class CategoryState {
    current = $state<Category>('video');

    constructor() {
        if (typeof localStorage === 'undefined') return;
        const saved = localStorage.getItem(LS_KEY);
        if (saved && (CATEGORIES as string[]).includes(saved)) this.current = saved as Category;
    }

    set(c: Category) {
        this.current = c;
        try {
            localStorage.setItem(LS_KEY, c);
        } catch {}
    }
}

export const category = new CategoryState();

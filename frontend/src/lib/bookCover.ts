const PROXY_COVER = /\/api\/books\/cover\/(\d+)(?:\.jpg)?(?:[?#]|$)/;
const LEGACY_PROXY_COVER = /\/api\/books\/cover\/\d+(?:[?#]|$)/;
const OPEN_LIBRARY_COVER = /\/b\/id\/(\d+)-[SML]\.jpg(?:[?#]|$)/i;

function coverId(url: string): string | null {
    return url.match(PROXY_COVER)?.[1] ?? url.match(OPEN_LIBRARY_COVER)?.[1] ?? null;
}

export function bookCoverSrc(url: string): string {
    const id = coverId(url);
    return id && PROXY_COVER.test(url) ? `/api/books/cover/${id}.jpg` : url;
}

export function bookCoverNeedsRefresh(url: string | null): boolean {
    return !url || LEGACY_PROXY_COVER.test(url);
}

export function retryBookCover(event: Event, originalUrl: string) {
    const image = event.currentTarget as HTMLImageElement;
    const id = coverId(originalUrl);

    if (id && !image.dataset.openLibraryFallback) {
        image.dataset.openLibraryFallback = 'true';
        image.src = `https://covers.openlibrary.org/b/id/${id}-L.jpg?default=false`;
        return;
    }

    image.hidden = true;
}

export function validateBookCover(event: Event, originalUrl: string) {
    const image = event.currentTarget as HTMLImageElement;
    if (image.naturalWidth > 2 && image.naturalHeight > 2) return;
    retryBookCover(event, originalUrl);
}

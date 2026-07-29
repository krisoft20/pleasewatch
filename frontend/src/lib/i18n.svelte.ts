import {
    lang as langStore,
    currentLang,
    setLang,
    tSync,
    LANGS as LANGS_legacy,
    subLangCode as subLangCode_legacy,
    flagEmoji as flagEmoji_legacy,
    type Lang as Lang_legacy
} from './i18n';

export type Lang = Lang_legacy;
export const LANGS = LANGS_legacy;
export const subLangCode = subLangCode_legacy;
export const flagEmoji = flagEmoji_legacy;

class I18n {
    lang = $state<Lang>(currentLang());

    constructor() {
        langStore.subscribe((v) => {
            if (v !== this.lang) this.lang = v;
        });
    }

    set(code: Lang) {
        setLang(code);
        this.lang = code;
    }
}

export const i18n = new I18n();

export function t(key: string, vars?: Record<string, unknown>): string {
    void i18n.lang;
    return tSync(key, vars);
}

const PLURAL_LOCALE: Record<Lang, string> = { EN: 'en', PL: 'pl', DE: 'de' };

export function plural(stem: string, n: number): string {
    void i18n.lang;
    const rule = new Intl.PluralRules(PLURAL_LOCALE[i18n.lang] ?? 'en').select(n);
    const primary = tSync(`${stem}.${rule}`, { n });
    if (primary !== `${stem}.${rule}`) return primary;
    return tSync(`${stem}.other`, { n });
}

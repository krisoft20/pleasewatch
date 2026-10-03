const BRAND: Record<string, string> = {
    alabama: '#9E1B32',
    georgia: '#BA0C2F',
    'ohio state': '#BB0000',
    michigan: '#00274C',
    'michigan state': '#18453B',
    texas: '#BF5700',
    'texas a&m': '#500000',
    oklahoma: '#841617',
    'oklahoma state': '#FF7300',
    lsu: '#461D7C',
    'ole miss': '#14213D',
    oregon: '#154733',
    washington: '#4B2E83',
    clemson: '#F56600',
    'florida state': '#782F40',
    florida: '#0021A5',
    tennessee: '#FF8200',
    auburn: '#0C2340',
    'notre dame': '#C99700',
    'penn state': '#041E42',
    wisconsin: '#C5050C',
    nebraska: '#E41C38',
    usc: '#990000',
    ucla: '#2D68C4',
    utah: '#CC0000',
    baylor: '#154734',
    tcu: '#4D1979',
    byu: '#002E5D',
    miami: '#F47321',
    louisville: '#AD0000',
    kentucky: '#0033A0',
    arkansas: '#9D2235',
    missouri: '#F1B82D',
    'south carolina': '#73000A',
    'virginia tech': '#630031',
    'nc state': '#CC0000',
    duke: '#003087',
    'north carolina': '#7BAFD4',
    kansas: '#0051BA',
    'kansas state': '#512888',
    'iowa state': '#C8102E',
    iowa: '#FFCD00',
    purdue: '#CEB888',
    illinois: '#E84A27',
    'boise state': '#0033A0',
    memphis: '#003087',
    cincinnati: '#E00122',
    chiefs: '#E31837',
    bills: '#00338D',
    '49ers': '#AA0000',
    cowboys: '#041E42',
    eagles: '#004C54',
    ravens: '#241773',
    lions: '#0076B6',
    packers: '#203731',
    bengals: '#FB4F14',
    browns: '#311D00',
    steelers: '#FFB612',
    dolphins: '#008E97',
    patriots: '#002244',
    jets: '#125740',
    texans: '#03202F',
    colts: '#002C5F',
    jaguars: '#006778',
    titans: '#4B92DB',
    broncos: '#FB4F14',
    chargers: '#0080C6',
    raiders: '#A5ACAF',
    giants: '#0B2265',
    commanders: '#5A1414',
    bears: '#0B162A',
    vikings: '#4F2683',
    falcons: '#A71930',
    panthers: '#0085CA',
    saints: '#D3BC8D',
    buccaneers: '#D50A0A',
    cardinals: '#97233F',
    rams: '#003594',
    seahawks: '#002244',
    celtics: '#007A33',
    lakers: '#552583',
    warriors: '#1D428A',
    knicks: '#006BB6',
    heat: '#98002E',
    nuggets: '#0E2240',
    thunder: '#007AC1',
    suns: '#1D1160',
    oilers: '#041E42',
    bruins: '#FFB81C',
    rangers: '#0038A8',
    avalanche: '#6F263D',
    dodgers: '#005A9C',
    yankees: '#132448',
    braves: '#CE1141',
    astros: '#EB6E1F',
    arsenal: '#EF0107',
    'man city': '#6CABDD',
    liverpool: '#C8102E',
    chelsea: '#034694',
    'real madrid': '#FEBE10',
    barcelona: '#A50044',
    'man united': '#DA291C'
};

const BRAND_KEYS = Object.keys(BRAND).sort((a, b) => b.length - a.length);

const STOP = new Set(['the', 'of', 'at', 'university', 'univ']);

function rgb(hex: string): [number, number, number] {
    const m = hex.replace('#', '');
    return [0, 2, 4].map((i) => parseInt(m.slice(i, i + 2), 16)) as [number, number, number];
}

export function lum(hex: string): number {
    const f = (v: number) => (v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4));
    const [r, g, b] = rgb(hex).map((v) => v / 255);
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function dist(a: string, b: string): number {
    const x = rgb(a);
    const y = rgb(b);
    return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]);
}

function lighten(hex: string, amt: number): string {
    const ch = (v: number) =>
        Math.round(v + (255 - v) * amt)
            .toString(16)
            .padStart(2, '0');
    return '#' + rgb(hex).map(ch).join('');
}

function darken(hex: string, amt: number): string {
    const ch = (v: number) =>
        Math.round(v * (1 - amt))
            .toString(16)
            .padStart(2, '0');
    return '#' + rgb(hex).map(ch).join('');
}

export function ink(hex: string): string {
    return lum(hex) > 0.179 ? '#08090b' : '#ffffff';
}

function norm(name: string): string {
    return name
        .toLowerCase()
        .replace(/[^a-z0-9 &]/g, '')
        .replace(/\s+/g, ' ')
        .trim();
}

function hashColor(name: string): string {
    let h = 2166136261;
    for (let i = 0; i < name.length; i++) {
        h ^= name.charCodeAt(i);
        h = Math.imul(h, 16777619);
    }
    const hue = Math.abs(h) % 360;
    const s = 0.58;
    const l = 0.4;
    const c = (1 - Math.abs(2 * l - 1)) * s;
    const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
    const m = l - c / 2;
    const seg: [number, number, number] =
        hue < 60
            ? [c, x, 0]
            : hue < 120
              ? [x, c, 0]
              : hue < 180
                ? [0, c, x]
                : hue < 240
                  ? [0, x, c]
                  : hue < 300
                    ? [x, 0, c]
                    : [c, 0, x];
    return (
        '#' +
        seg
            .map((v) =>
                Math.round((v + m) * 255)
                    .toString(16)
                    .padStart(2, '0')
            )
            .join('')
    );
}

export function teamColor(name: string): string {
    const n = norm(name);
    if (!n) return '#5a5a63';
    const hit = BRAND_KEYS.find((k) => n === k || n.includes(k));
    return hit ? BRAND[hit] : hashColor(n);
}

export function pairColors(a: string, b: string): [string, string] {
    const ca = teamColor(a);
    let cb = teamColor(b);
    if (dist(ca, cb) < 78) cb = lum(cb) < 0.34 ? lighten(cb, 0.4) : darken(cb, 0.44);
    return [ca, cb];
}

export function abbrOf(name: string): string {
    const raw = name.trim();
    if (!raw) return '--';
    const caps = raw.match(/\b[A-Z]{3,4}\b/);
    if (caps) return caps[0];
    const words = raw.split(/\s+/).filter((w) => /^[a-z]/i.test(w) && !STOP.has(w.toLowerCase()));
    if (words.length === 0) return raw.slice(0, 3).toUpperCase();
    if (words.length === 1 || words.length > 3) return words[0].slice(0, 4).toUpperCase();
    const initials = words
        .map((w) => w[0])
        .join('')
        .toUpperCase();
    return initials.length >= 3 ? initials : words[0].slice(0, 3).toUpperCase();
}

export function countdown(mins: number | null): string {
    if (mins === null || mins < 0) return '';
    if (mins < 60) return `in ${mins}m`;
    if (mins < 60 * 24) {
        const h = Math.floor(mins / 60);
        const m = mins % 60;
        return m ? `in ${h}h ${m}m` : `in ${h}h`;
    }
    const d = Math.round(mins / (60 * 24));
    return d <= 1 ? 'tomorrow' : `in ${d}d`;
}

export function darkLogo(url: string | null): string | null {
    return url ? url.replace('/500/', '/500-dark/') : null;
}

const FAV_KEY = 'pw-live-teams';

export type FavTeam = { name: string; color?: string; logo?: string; league?: string };

class FavTeams {
    teams = $state<FavTeam[]>([]);

    constructor() {
        if (typeof localStorage === 'undefined') return;
        try {
            const raw = JSON.parse(localStorage.getItem(FAV_KEY) ?? '[]');
            if (!Array.isArray(raw)) return;
            this.teams = raw
                .map((x) => (typeof x === 'string' ? { name: x } : x))
                .filter((x): x is FavTeam => !!x && typeof x.name === 'string' && !!x.name);
        } catch {}
    }

    get names(): string[] {
        return this.teams.map((t) => t.name);
    }

    has(name: string): boolean {
        const n = norm(name);
        if (!n) return false;
        return this.teams.some((f) => {
            const g = norm(f.name);
            return n === g || n.includes(g) || g.includes(n);
        });
    }

    toggle(team: FavTeam) {
        const clean = team.name?.trim();
        if (!clean) return;
        this.teams = this.has(clean)
            ? this.teams.filter((f) => norm(f.name) !== norm(clean))
            : [...this.teams, { ...team, name: clean }];
        this.save();
    }

    remove(name: string) {
        this.teams = this.teams.filter((f) => norm(f.name) !== norm(name));
        this.save();
    }

    private save() {
        try {
            localStorage.setItem(FAV_KEY, JSON.stringify(this.teams));
        } catch {}
    }
}

export const favTeams = new FavTeams();

export const livePick = $state<{ id: string | null }>({ id: null });

export type Family = { key: string; label: string; tint: string };

export const FAMILIES: Family[] = [
    { key: 'soccer', label: 'Soccer', tint: '#3ddc84' },
    { key: 'football', label: 'Football', tint: '#f0873a' },
    { key: 'basketball', label: 'Basketball', tint: '#ff7a45' },
    { key: 'baseball', label: 'Baseball', tint: '#5aa9ff' },
    { key: 'hockey', label: 'Hockey', tint: '#7fd4ff' },
    { key: 'combat', label: 'Combat', tint: '#ff5a5a' },
    { key: 'motor', label: 'Motorsport', tint: '#ffd34d' },
    { key: 'handball', label: 'Handball', tint: '#c48bff' },
    { key: 'volleyball', label: 'Volleyball', tint: '#ffb347' },
    { key: 'other', label: 'Other', tint: '#b9b9c2' }
];

const FAMILY_RULES: [RegExp, string][] = [
    [/^(nfl|cfb|ncaaf|xfl|ufl|cfl)$/, 'football'],
    [/^(nba|ncaab|ncaaw|wnba|euroleague|nbl)$|^fiba/, 'basketball'],
    [/^(mlb|npb|kbo|milb)$/, 'baseball'],
    [/^(nhl|khl|ahl)$/, 'hockey'],
    [/^(mma|ufc|boxing|bellator|wwe|aew|pfl|one)$/, 'combat'],
    [/^(f1|f2|f3|motogp|nascar|indycar|wrc|formula)/, 'motor'],
    [/^handball$/, 'handball'],
    [/^volleyball$/, 'volleyball'],
    [
        /^(premier-league|laliga|la-liga|serie-a|serie-b|bundesliga|ligue-1|championship|womens-super-league|mls|eredivisie|primeira-liga|liga-mx|ekstraklasa|super-lig|scottish-premiership|segunda|ucl|champions-league|europa-league|conference-league|world-cup|euros|copa|soccer)/,
        'soccer'
    ]
];

export function familyOf(sport: string): string {
    const s = sport.toLowerCase();
    for (const [re, fam] of FAMILY_RULES) if (re.test(s)) return fam;
    return 'other';
}

export function familyOfGame(g: { sport: string; league?: string }): string {
    const bySport = familyOf(g.sport);
    if (bySport !== 'other') return bySport;
    const slug = (g.league ?? '').toLowerCase().replace(/[^a-z0-9]+/g, '-');
    return slug ? familyOf(slug) : 'other';
}

const CALLSIGNS: [RegExp, string, boolean][] = [
    [/thestreameast\./i, 'Alpha', false],
    [/livetv\./i, 'Bravo', true]
];

export function sourceOf(url: string): { name: string; mirrors: boolean } {
    for (const [re, name, mirrors] of CALLSIGNS) {
        if (re.test(url)) return { name, mirrors };
    }
    return { name: 'Charlie', mirrors: false };
}

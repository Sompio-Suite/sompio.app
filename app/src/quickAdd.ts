// "Dentist Tue 15:00" → a reminder. Understands a time, a day (today, tomorrow, a weekday or a
// date) and simple repeats ("every Monday", "every day", "weekdays"), in English and Dutch.

export interface QuickRow { days: string; time: string; title: string }
export type Quick = { ok: true; row: QuickRow; when: Date; repeat: string | null } | { ok: false; reason: string };

const WEEKDAYS: [RegExp, number, string][] = [
  [/^(mon|monday|maandag)$/i, 1, "Mon"],
  [/^(tue|tues|tuesday|dinsdag)$/i, 2, "Tue"],
  [/^(wed|weds|wednesday|woensdag)$/i, 3, "Wed"],
  [/^(thu|thur|thurs|thursday|donderdag)$/i, 4, "Thu"],
  [/^(fri|friday|vrijdag)$/i, 5, "Fri"],
  [/^(sat|saturday|zaterdag)$/i, 6, "Sat"],
  [/^(sun|sunday|zondag)$/i, 0, "Sun"],
];
const MONTHS = ["jan", "feb", "ma", "apr", "m", "jun", "jul", "aug", "sep", "o", "nov", "dec"];
const MONTH_RE = /^(jan(uari|uary)?|feb(ruari|ruary)?|mar(ch)?|maart|mrt|apr(il)?|may|mei|jun[ei]?|jul[iy]?|aug(ustus|ust)?|sep(t|tember)?|o[ck]t(ober)?|nov(ember)?|dec(ember)?)\.?$/i;
const FILLER = /^(at|on|om|op|the|de|het|a|an|een|in)$/i;

const pad = (n: number) => String(n).padStart(2, "0");
const isoDay = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

function monthIndex(word: string): number {
  const w = word.toLowerCase().replace(".", "");
  if (/^(mar|maart|mrt)/.test(w)) return 2;
  if (/^(may|mei)/.test(w)) return 4;
  if (/^o[ck]t/.test(w)) return 9;
  return MONTHS.findIndex((m) => m.length >= 3 && w.startsWith(m));
}

export function parseQuick(input: string, now = new Date()): Quick {
  const text = input.trim();
  if (!text) return { ok: false, reason: "" };
  let words = text.split(/\s+/);
  const used = new Set<number>();
  let hours: number | null = null;
  let mins = 0;
  let date: Date | null = null;
  let repeat: string | null = null;

  // Time: 15:00, 15.30, 3pm, 3:30pm, 15u
  words.forEach((w, i) => {
    if (hours !== null) return;
    const lower = w.toLowerCase().replace(/[,]$/, "");
    let m = /^(\d{1,2})[:.](\d{2})(am|pm)?$/.exec(lower);
    if (m) {
      hours = Number(m[1]) % 24; mins = Number(m[2]);
      if (m[3] === "pm" && hours < 12) hours += 12;
      if (m[3] === "am" && hours === 12) hours = 0;
      used.add(i);
      return;
    }
    m = /^(\d{1,2})(am|pm|u)$/.exec(lower);
    if (m) {
      hours = Number(m[1]) % 24;
      if (m[2] === "pm" && hours < 12) hours += 12;
      if (m[2] === "am" && hours === 12) hours = 0;
      used.add(i);
      return;
    }
    if (/^(\d{1,2})$/.test(lower) && i + 1 < words.length && /^(am|pm)$/i.test(words[i + 1])) {
      hours = Number(lower) % 24;
      if (/pm/i.test(words[i + 1]) && hours < 12) hours += 12;
      used.add(i).add(i + 1);
    }
  });
  if (hours === null || hours > 23 || mins > 59) return { ok: false, reason: 'Add a time, like "Dentist Tue 15:00".' };

  // Repeats: every day, daily, weekdays, every Monday
  for (let i = 0; i < words.length; i++) {
    const w = words[i].toLowerCase();
    if (/^(daily|dagelijks)$/.test(w)) { repeat = "Every day"; used.add(i); }
    else if (/^(weekdays|werkdagen)$/.test(w)) { repeat = "Weekdays"; used.add(i); }
    else if (/^(weekends?)$/.test(w) && i > 0 && /^(every|elk|elke)$/i.test(words[i - 1])) { repeat = "Weekends"; used.add(i).add(i - 1); }
    else if (/^(every|elke|iedere|elk)$/.test(w) && i + 1 < words.length) {
      const next = words[i + 1].replace(/,$/, "");
      if (/^(day|dag)$/i.test(next)) { repeat = "Every day"; used.add(i).add(i + 1); }
      const wd = WEEKDAYS.find(([re]) => re.test(next.replace(/s$/i, "")) || re.test(next));
      if (wd) { repeat = wd[2]; used.add(i).add(i + 1); }
    }
  }

  // A day: today, tomorrow, a weekday, or a date
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 12);
  if (!repeat) {
    for (let i = 0; i < words.length && !date; i++) {
      if (used.has(i)) continue;
      const w = words[i].replace(/[,]$/, "");
      if (/^(today|vandaag)$/i.test(w)) { date = today; used.add(i); continue; }
      if (/^(tomorrow|tmrw|morgen)$/i.test(w)) { date = new Date(today); date.setDate(date.getDate() + 1); used.add(i); continue; }
      const wd = WEEKDAYS.find(([re]) => re.test(w));
      if (wd) {
        date = new Date(today);
        const ahead = (wd[1] - today.getDay() + 7) % 7;
        date.setDate(date.getDate() + ahead);
        used.add(i);
        continue;
      }
      let m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(w);
      if (m) { date = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]), 12); used.add(i); continue; }
      m = /^(\d{1,2})[/-](\d{1,2})(?:[/-](\d{2,4}))?$/.exec(w);
      if (m) {
        const y = m[3] ? (m[3].length === 2 ? 2000 + Number(m[3]) : Number(m[3])) : today.getFullYear();
        date = new Date(y, Number(m[2]) - 1, Number(m[1]), 12);
        if (!m[3] && date < today) date.setFullYear(y + 1);
        used.add(i);
        continue;
      }
      // "18 nov" or "nov 18"
      const next = words[i + 1]?.replace(/[,]$/, "");
      if (/^\d{1,2}$/.test(w) && next && MONTH_RE.test(next)) {
        date = new Date(today.getFullYear(), monthIndex(next), Number(w), 12);
        if (date < today) date.setFullYear(date.getFullYear() + 1);
        used.add(i).add(i + 1);
        continue;
      }
      if (MONTH_RE.test(w) && next && /^\d{1,2}(st|nd|rd|th)?$/.test(next)) {
        date = new Date(today.getFullYear(), monthIndex(w), parseInt(next, 10), 12);
        if (date < today) date.setFullYear(date.getFullYear() + 1);
        used.add(i).add(i + 1);
      }
    }
  }

  const title = words
    .filter((_, i) => !used.has(i))
    .filter((w, i, all) => !(FILLER.test(w) && (i === all.length - 1 || i === 0 || FILLER.test(all[i + 1] ?? ""))))
    .join(" ")
    .replace(/\s+(at|on|om|op)$/i, "")
    .replace(/^[,\-–\s]+|[,\-–\s]+$/g, "");
  if (!title) return { ok: false, reason: "What's it for? Add a few words, like \"Dentist\"." };

  const time = `${pad(hours)}:${pad(mins)}`;
  if (repeat) {
    const when = new Date(today);
    when.setHours(hours, mins, 0, 0);
    return { ok: true, row: { days: repeat, time, title }, when, repeat };
  }
  if (!date) {
    // No day given: today if the time is still to come, otherwise tomorrow
    date = new Date(today);
    if (hours * 60 + mins <= now.getHours() * 60 + now.getMinutes()) date.setDate(date.getDate() + 1);
  } else if (isoDay(date) === isoDay(today) && hours * 60 + mins <= now.getHours() * 60 + now.getMinutes()) {
    // "Mon 9:00" said on a Monday afternoon means next Monday
    const said = words.some((w) => WEEKDAYS.some(([re]) => re.test(w)));
    if (said) date.setDate(date.getDate() + 7);
  }
  const when = new Date(date);
  when.setHours(hours, mins, 0, 0);
  return { ok: true, row: { days: isoDay(date), time, title }, when, repeat: null };
}

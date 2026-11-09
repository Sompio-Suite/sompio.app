import type { RowInput } from "./api";

export interface Pack { id: string; name: string; line: string; rows: RowInput[] }

/** Ready-made routines. Each adds plain reminders to your schedule that you can change afterwards. */
export const PACKS: Pack[] = [
  {
    id: "privacy",
    name: "Privacy check-ups",
    line: "A few minutes a month that keep your accounts and data tidy.",
    rows: [
      { days: "Monthly 1", time: "19:00", title: "Password check-up", text: "Change one old password, and check you don't use it anywhere else.", notice: 0 },
      { days: "Monthly 15", time: "19:00", title: "Review what you shared", text: "Open Sompio Clean and go through anything new it found.", notice: 0 },
      { days: "Monthly 20", time: "19:00", title: "Check app permissions", text: "On your phone: which apps can see your location, camera, microphone and contacts? Turn off what they don't need.", notice: 0 },
      { days: "Quarterly 1", time: "18:30", title: "Request fresh data exports", text: "Ask Discord, Google, Meta and others for a copy of your data, so Clean can check it when it arrives.", notice: 0 },
      { days: "Quarterly 10", time: "18:30", title: "Sign out of old sessions", text: "Check the logged-in devices on your main accounts and sign out of the ones you don't know or use.", notice: 0 },
      { days: "Quarterly 25", time: "18:30", title: "Is your recovery code still safe?", text: "Check you can still find your Sompio recovery sheet and a copy of your key picture.", notice: 0 },
    ],
  },
  {
    id: "water",
    name: "Drink water",
    line: "Every hour through the day.",
    rows: [{ days: "Every day", time: "every 60 min 09:00-21:00", title: "Drink water", text: "Have a glass of water (250 ml)." }],
  },
  {
    id: "move",
    name: "Move",
    line: "Get up once an hour.",
    rows: [{ days: "Every day", time: "every 60 min 10:30-20:30", title: "Move", text: "Stand up, stretch, walk around for a minute." }],
  },
  {
    id: "eyes",
    name: "Eye breaks",
    line: "The 20-20-20 rule while you're at the screen.",
    rows: [{ days: "Weekdays", time: "every 20 min 09:20-18:00", title: "Eye break", text: "Look at something 6 metres away for 20 seconds." }],
  },
  {
    id: "meals",
    name: "Meals",
    line: "Breakfast, lunch and dinner, with a heads-up before each.",
    rows: [
      { days: "Every day", time: "08:00", title: "Breakfast", text: "Time to eat something.", notice: 15 },
      { days: "Every day", time: "12:30", title: "Lunch", text: "Take a proper break.", notice: 15 },
      { days: "Every day", time: "18:00", title: "Dinner", text: "Time for dinner.", notice: 15 },
    ],
  },
  {
    id: "bed",
    name: "Bedtime",
    line: "Wind down, with half an hour's warning.",
    rows: [{ days: "Every day", time: "23:00", title: "Bedtime", text: "Screens off. Sleep well.", notice: 30 }],
  },
];

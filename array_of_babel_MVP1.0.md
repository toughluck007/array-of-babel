# **Array of Babel**  
### Game Design Document (GDD)  
*Author:* <Your Name>  
*Version:* 0.1 (MVP)  
*Date:* 2025-09-17

---

## Table of Contents

1. High Concept  
2. Game Pillars  
3. Core Loop (MVP)  
4. Systems  
   1. Jobs  
   2. Processors  
   3. Resolution Math  
   4. Automation (“Daemon”)  
   5. Economy  
   6. Data Storage & Lore Feature Hook  
   7. Progression  
5. UI / Layout  
6. Controls & Input  
7. Data & Content Structuring  
8. Technical Architecture  
   1. Project Structure  
   2. Concurrency & Timing  
   3. Persistence / Save / Load  
   4. UI Frameworks / Crates  
9. MVP Acceptance Criteria  
10. Expansion Roadmap  
11. Lore & Flavor Ideas  

---

## 1. High Concept

You are the master interface of a nascent supercomputer known as the **Array of Babel**, managing an array of disparate processors. Jobs arrive, you assign them to processors, you earn credits, you grow the machine. Everything is numbers + text in a single TUI, with popovers for job/store/etc. Start simple; build lore, features, and complexity later.

---

## 2. Game Pillars

- **Simplicity**: one screen + popovers, clear numerical feedback.  
- **Numeric fidelity**: speed, quality, instruction compatibility matter.  
- **Modular expansion**: processor/job types, storage, lore live in data (e.g. RON/TOML).  
- **Trade‑offs matter**: automation ≠ strictly better; storage cost, quality, etc.

---

## 3. Core Loop (MVP)

1. Jobs appear in the “Jobs” panel/popover.  
2. Player selects a job → pending queue.  
3. Player assigns to a compatible free processor → timer begins.  
4. When complete, payout is computed (based on quality, speed) → credits added, processor freed.  
5. Player can open Store to buy upgrades (speed, quality bias, instruction tag additions).  
6. At some threshold, unlock automation (“Daemon”) with associated penalties.  
7. Repeat, trying to maximize throughput / credits over “days” (tick units).

---

## 4. Systems

### 4.1 Jobs

- Fields:  
  - `id: String`  
  - `type/tag: String` (e.g., “GENERAL”, “SIMD”, “FP64”, etc.)  
  - `base_time: Duration`  
  - `base_reward: u64`  
  - `quality_target: u8` (0–100)  
  - optionally `deadline: Duration` (for later)  

- Generation:  
    Job tiers: early only GENERAL; later include more tags.  
    Random noise in payout or time/quality so things vary.

- Compatibility: only processors with matching tags/instruction support can run.

### 4.2 Processors

- Fields:  
    `name: String`  
    `speed: f64` multiplier (base = 1.0)  
    `quality_bias: i8` (could be negative)  
    `instruction_set: Vec<String>`  
    `upkeep_cost: u64` per “day” (tick)  

- Starter processor: **Model F12‑Scalar**  
    speed = 1.0, quality_bias = 0, instruction_set = ["GENERAL"], upkeep low.  

- Future processors: “Vanta‑GPU”, “Hermes‑PPU”, etc.

### 4.3 Resolution Math

- **Time to complete** = `base_time / speed`.  
- **Quality** = `quality_target + quality_bias + ε`, clamped to [0, 100], where ε is small random noise.  
- **Payout** = `base_reward * f(quality)`, where f might be linear from quality 0‑100 mapping to say 0.7× to 1.2× reward.  
- (Later) **Penalty if late**: some reduction if past deadline.

### 4.4 Automation (“Daemon”)

- Unlocks after some credit threshold (e.g. 500 credits).  
- When on, idle processors will auto‑pick jobs (lowest risk / easiest first).  
- Penalties: reduced quality or slower processing (e.g. −5 quality, +10% time).  

### 4.5 Economy

- Currencies: **Credits**.  
- Upgrades purchasable in Store:  
    • Clock Tuning (+speed small increments)  
    • Calibration (+quality)  
    • Instruction Microcode (add new instruction tags)  
- Upkeep: each processor costs upkeep per day; ensures you need to run jobs consistently.

### 4.6 Data Storage & Lore Feature Hook

- **Stored Data Metric**: Jobs have a “filesize” (e.g. data_units), representing how much data was processed/stored.  
- **Storage Capacity**: you have a cap; initial cap small. You can buy more capacity.  
- **Passive Income Potential**: stored data can be “licensed” or “sold” or otherwise generate passive income (e.g. a % of data_units * a rate per day).  
- **Lore tie‑ins**: the Array of Babel gathers data, research, knowledge; sells access or publishes reports; maybe some job types pertain to archives / datasets.  

### 4.7 Progression

- Day Tiers: each “day” or milestone unlocks things: new job tag, new processor model slot, capacity expansion.  
- Milestones: e.g. first specialty processor, first large job, first 1,000 data_units stored.

---

## 5. UI / Layout

```
+---------------------------------------------------------------+
| Array of Babel              Day: ___    Time: ___   Credits: __|
+---------------------+----------------------+------------------+
| Processors Panel    | Jobs Panel           | Data Storage &   |
| (list, status, ETA) | (pending + running)  | Stats / Lore     |
+---------------------+----------------------+------------------+
| Footer / Hotkeys                                            |
+---------------------------------------------------------------+
```

- **Popovers/Overlays**:  
  • Jobs picker/popover (show full list; select job)  
  • Store popover (upgrade options)  
  • Data Storage / Stats popover (show data_units, capacity, passive income etc.)  

- **Panels**:  
  Left: Processors; Middle: Jobs; Right: Data Storage / Stats (or show stats in top bar if screen narrow)  

- **Footer / Hotkeys**: keys like [J] (Jobs), [A] (Assign), [S] (Store), [D] (Daemon toggle), [C] (Capacity upgrade), [Q] (Quit).  

---

## 6. Controls & Input

- Arrow keys or `j/k` for navigation inside lists or between items.  
- `Tab` or similar to switch panel focus.  
- `Enter` to select / assign.  
- Hotkeys to bring up popovers:  
    J – jobs popover  
    S – store  
    D – toggle daemon  
    C – capacity/store data storage upgrades  
    Q – quit / save  

---

## 7. Data & Content Structuring

- Use **RON** or **TOML** files for:  
    • Processor definitions  
    • Job template definitions  
    • Store items / upgrades  
    • Lore text / flavor text  

- All new content should be possible to add by editing data files only (no needing to recompile logic, unless logic needs new hooks).

---

## 8. Technical Architecture

### 8.1 Project Structure

```
array_of_babel/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── ui/
│   │   ├── processors_view.rs
│   │   ├── jobs_view.rs
│   │   ├── storage_view.rs
│   │   ├── store_view.rs
│   ├── sim/
│   │   ├── jobs.rs
│   │   ├── processors.rs
│   │   ├── economy.rs
│   │   ├── data_storage.rs
│   ├── persist/
│   │   ├── save.rs
│   │   ├── load.rs
│   ├── data/        # for RON/TOML files
│   └── lore/        # flavor text, events
└── assets/          # if later needed (optional)
```

### 8.2 Concurrency & Timing

- Use **Tokio** for async runtime.  
- UI loop: fixed tick rate (e.g. 10‑30 fps) for rendering and input.  
- Sim tasks: each job assigned spawns a timer (async sleep) to simulate work.  

### 8.3 Persistence / Save / Load

- On quit or end of “day”, serialize game state (processors, pending/running jobs, credits, stored data, upgrades) via Serde into a file (RON).  
- On launch, check for existing save, load if present.  

### 8.4 UI Framework & Dependencies

- **ratatui** for terminal UI.  
- **crossterm** for terminal backend and event handling.  
- **tokio** for async tasks/timers.  
- **serde** + **ron** (or toml) for data and save serialization.  
- (Optionally) **color‑eyre** for error reporting.  

---

## 9. MVP Acceptance Criteria

- The application runs; one processor (Model F12‑Scalar) exists.  
- Jobs appear (GENERAL type), you can view job list.  
- Manual assignment of job → processor works; progress of job is visible.  
- Completion works; payout is correct; credits accumulate.  
- Store has at least two upgrade options (speed, quality). Purchases reflect in processor stats.  
- Daemon unlocks at some credit milestone; when turned on, auto‑assignment works with its penalties.  
- Data storage feature: storage capacity exists; data_units from completed jobs accumulate; capacity can be expanded; (optional) passive income from stored data starts working.  
- Save and load functionality works correctly.

---

## 10. Expansion Roadmap

- Introduce new job tags (SIMD, FP64, RT, etc.).  
- Introduce specialist processors.  
- Deadlines and penalties for late jobs.  
- Contracts / multi‑stage jobs.  
- Lore events that alter job feed (e.g. “Data Surge”, “Cache Storm”).  
- Marketplace / licensing feature for stored data.  
- UI enhancements: stats/light visual gauges, charts.  

---

## 11. Lore & Flavor Ideas

- The **Array of Babel** is a grand project archiving research, knowledge, and computation.  
- Jobs are framed as “research briefs”, “data archives”, “signal processing”, “rendering anomalies”, etc.  
- Stored data corresponds to archives that can later be accessed / sold for licenses, or accessed by other entities.  
- Processor models have lore: e.g. “Hermes‑PPU”: built to simulate photon paths in archaic observatories. “Quill‑DSP”: used originally in sound‑scape mapping.  


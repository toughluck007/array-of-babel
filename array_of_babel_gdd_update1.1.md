# Array of Babel — Game Design Document (v0.2)

## Scope of this revision

Adds systemic mechanics for **Reliability**, **Cooling & Burnout**, **Hardening**, **Fragility/Finite Lifespan**, **Replacement flows**, and **Electricity/Upkeep interactions**. Keeps content-first approach while minimizing risk to core loop.

---

## 0) Quick Summary for Codex (dev overview)

- Add per-processor stats: `reliability_base`, `cooling_required` (bool), `cooling_level` (0..n), `hardening_level` (0..n), `finite_lifespan` (bool), `mean_time_to_failure` (MTTF ticks), `power_draw_base`, `power_draw_mod`, `heat_output_base`.
- Compute **Burnout Risk** per job tick from heat & load → failure rolls.
- Add **Cooling** as an upgrade tree. Cooling mitigates heat, improves reliability, reduces failure chance; some models *require* minimum cooling to operate safely.
- Add **Hardening** as per-processor (or global facility) add-on that reduces radiation/ANGEL/surveillance corruption and improves reliability in hostile domains.
- Support **finite/fragile** models that will eventually break (consumables). Provide **Replace** and **Replace All** flows with partial-cost pricing.
- Persist new fields in save-game; extend store to sell cooling/hardening/replace.

---

## 1) New/Updated Stats (per Processor Instance)

| Field                  | Type          | Default | Notes                                                                      |
| ---------------------- | ------------- | ------- | -------------------------------------------------------------------------- |
| `reliability_base`     | f64 (0..1)    | 0.995   | Baseline success chance per job **without** heat/overload effects.         |
| `reliability_current`  | f64 (0..1)    | derives | Derived from base ± modifiers.                                             |
| `cooling_required`     | bool          | false   | If true, apply heavy penalties when cooling\_level == 0.                   |
| `cooling_level`        | u8            | 0       | Each level reduces heat factor and raises reliability.                     |
| `cooling_cap`          | u8            | 3       | Max level; models may vary.                                                |
| `hardening_level`      | u8            | 0       | Reduces radiation/ANGEL/surveillance corruption & burnout from those tags. |
| `power_draw_base`      | f64           | model   | kWh per “day” (tick-window).                                               |
| `power_draw_mod`       | map\<Tag,f64> | {}      | Multiplier applied when running certain tags (e.g., `SIMD:+0.5`).          |
| `heat_output_base`     | f64           | model   | Feeds burnout model; rises with overclock/overload.                        |
| `finite_lifespan`      | bool          | false   | If true, the device has a wear-out curve.                                  |
| `mttf_ticks`           | u64           | 0       | Mean-time-to-failure when `finite_lifespan` is true.                       |
| `wear`                 | f64 (0..1)    | 0.0     | Accumulates; at 1.0 device is destroyed.                                   |
| `fragility`            | f64           | 0.0     | Extra failure risk spike from shocks (e.g., heat spikes, ANGEL).           |
| `replace_cost_ratio`   | f64           | 0.35    | % of full price when using Replace/Replace All.                            |
| `requires_cooling_min` | u8            | 0       | Minimum safe cooling to avoid severe penalties.                            |

### Store Upgrades (new)

- **Cooling Kit I/II/III** → +1 cooling\_level per purchase up to cap. Price scales per slot.
- **Hardening Module I/II** → +1 hardening\_level up to cap (global or per unit—choose one per design).
- **Service-Grade Thermal Paste** → temporary buff (session/day) to heat dissipation.
- **Replace (single)** / **Replace All (brand/model)** → spawns fresh instances at `replace_cost_ratio`.

---

## 2) Burnout, Failure & Reliability Model

**Goal:** simple, legible, tunable.

### 2.1 Heat & Load

For a job with tags `T` and processor `P`:

- \(power\_draw = power\_draw\_base * (1 + \sum power\_draw\_mod[T])\)
- \(heat = heat\_output\_base * f_{overload}(T, speed, SIMD\_util) - cooling\_mitigation(cooling\_level)\)

Where:

- `f_overload` rises with high-util tags (e.g., `SIMD`, `PHOTONIC`).
- `cooling_mitigation(level)` is stepwise or smooth (e.g.,\(0.25, 0.45, 0.60\) reductions).

### 2.2 Reliability per Tick

Base reliability adjusted by heat, tag hazards, and add-ons:

$$
rel_{tick} = clamp( reliability\_base
 - k_h * heat
 - k_t * tag\_hazard(T)
 + k_c * cooling\_bonus(cooling\_level)
 + k_r * hardening\_bonus(hardening\_level, T)
 , 0, 1)
$$

- `tag_hazard(T)`: e.g., `RADIATION` = 0.02, `ANGEL` = 0.03, `SURVEILLANCE` = 0.01, else 0.
- `hardening_bonus` reduces hazard, especially for `RADIATION`/`ANGEL`.

**Tick failure roll:** Burnout occurs if `rand() > rel_tick` during any work tick. On failure: processor becomes **Burnt Out**.

### 2.3 Wear & Finite Lifespan

If `finite_lifespan`:

- Increment `wear` per tick: \(\Delta wear = base\_wear + heat\_wear * heat + hazard\_wear(T)\)
- Failure when `wear >= 1.0` → **Destroyed** (removed from roster). Some brands (Heretic, Hermes) may use higher `base_wear` or spike from events.

---

## 3) States & Outcomes

- **OK** → operating normally.
- **Overheating** → performance debuff (−speed, −quality) and higher failure risk.
- **Burnt Out** → non-functional; offers **Replace** (**R**) or **Replace All** (**Shift+R**) shortcuts.
- **Destroyed** → removed; only **Purchase New** flow available.

**UI/UX:**

- Iconography: thermometer (heat), shield (hardening), spark/skull (burnout), hourglass (wear).
- Tooltips: show current `rel_tick`, heat level, hazard mods, and ETA impact.

---

## 4} Replacement & Pricing

- **Replace (one):** removes burnt unit; spawns identical model at `price * replace_cost_ratio` (default 35%). Resets `wear`, keeps no upgrades unless specified (optionally refund % of attached upgrades or charge extra to re-fit).
- **Replace All (model filter):** applies the above to all burnt units of the same model. Confirmation modal shows total cost and net change.
- Optional: **Service Contract** upgrade reduces `replace_cost_ratio` by 5–15%.

---

## 5) Electricity & Upkeep Integration

- Daily electricity cost: \(cost_{power} = kWh\_cost * \sum power\_draw\_{effective}\)
- Upkeep now includes `cost_power + maintenance_cost`.
- Cooling increases power draw slightly; hardening has fixed maintenance cost.

---

## 6) Data Structures (RON/TOML sketches)

### 6.1 Processor Model Definition (static)

```toml
# data/processors/vek_ember_32.toml
[id]
brand = "Vek"
model = "Ember-32"
price = 320
speed = 1.25
quality_bias = -2
instruction_set = ["GENERAL","SIMD"]
power_draw_base = 1.4
heat_output_base = 1.2
reliability_base = 0.985
cooling_required = true
requires_cooling_min = 1
cooling_cap = 3
hardening_cap = 1
finite_lifespan = false
replace_cost_ratio = 0.35
```

### 6.2 Processor Instance (save-state)

```ron
(
  brand: "Vek",
  model: "Ember-32",
  speed: 1.25,
  quality_bias: -2,
  cooling_level: 0,
  hardening_level: 0,
  wear: 0.07,
  state: Ok,
)
```

### 6.3 Global Tunables (balances.toml)

```toml
[burnout]
k_h = 0.08
k_t = 1.0
k_c = 0.05
k_r = 0.06

[tag_hazard]
RADIATION = 0.02
ANGEL = 0.03
SURVEILLANCE = 0.01

[cooling_mitigation]
# fraction reductions applied to heat
level1 = 0.25
level2 = 0.45
level3 = 0.60

[power]
kwh_cost = 4.0  # credits per kWh
```

---

## 7) Simulation Hooks (pseudocode)

```rust
fn step_job_tick(proc: &mut Processor, job: &Job, dt: Tick) {
  let power = effective_power(proc, job);
  let heat  = effective_heat(proc, job);

  // Reliability per tick
  let rel_tick = (proc.reliability_base
                 - KH * heat
                 - KT * tag_hazard(job.tags)
                 + KC * cooling_bonus(proc.cooling_level)
                 + KR * hardening_bonus(proc.hardening_level, job.tags))
                 .clamp(0.0, 1.0);

  if rng() > rel_tick {
     proc.state = State::BurntOut;
     emit(Event::Burnout{proc_id: proc.id, job_id: job.id});
     return;
  }

  // Wear progression
  if proc.finite_lifespan {
     proc.wear += wear_delta(proc, heat, job.tags, dt);
     if proc.wear >= 1.0 {
        proc.state = State::Destroyed;
        emit(Event::Destroyed{proc_id: proc.id});
        return;
     }
  }

  // Normal progress
  advance_job(job, proc, dt);
}
```

---

## 8) Store / UI Additions

- **Cooling tab**: list processors → upgrade button per unit (+ level). Multi-select for batch upgrades.
- **Hardening tab**: (global or per unit). Describe domain effects (Radiation/ANGEL/etc.).
- **Replace actions**: in Processor list context menu: `R` Replace, `Shift+R` Replace All (same model). Modal with cost breakdown.
- **Indicators**: top bar shows total power draw and electricity cost/day.

---

## 9) Content Notes (lore-friendly defaults)

- Vek: `cooling_required=true`, low reliability\_base, high speed; benefits a lot from Cooling.
- Oort: `hardening_level` cap high, great vs Radiation; slower, costly.
- Hermes: high heat on PHOTONIC; fragile → finite\_lifespan=true on some models.
- Heretic: low reliability\_base + high fragility; huge performance upside; frequent Replace flow.
- Mnemosyne: storage/compression focus; low heat; very high reliability\_base.

---

## 10) Acceptance Criteria (v0.2)

1. New stats serialized in save/load; defaults applied to existing saves.
2. Burnout rolls trigger under heat/hazard conditions; processors can become Burnt Out/Destroyed.
3. Cooling upgrades reduce burnout incidence and increase reliability; min cooling enforced where required.
4. Hardening reduces hazard penalties and data corruption risk on relevant tags.
5. Replace and Replace All actions work; pricing uses `replace_cost_ratio` with correct totals.
6. Electricity cost UI shows updated totals factoring in power draw, cooling impact.

---

## 11) Future Hooks

- Cooling network capacity & shared chillers.
- Scheduled maintenance to reset wear.
- Facility-wide hardening tiers with area-of-effect.
- Collector achievements for surviving fragile devices beyond MTTF.

---

## 12) Per‑Processor Daemon Automation (replaces global toggle)

### Goals

- Make automation a **slot-level decision** with meaningful trade‑offs.
- Preserve readability: clear indicators per processor; predictable behavior.

### New Fields (per Processor Instance)

| Field             | Type                     | Default | Notes                                                                     |
| ----------------- | ------------------------ | ------- | ------------------------------------------------------------------------- |
| `daemon_mode`     | enum {Off, Assist, Auto} | Off     | Off = manual only; Assist = suggest jobs; Auto = self‑assign.             |
| `daemon_affinity` | map\<Tag,f64>            | {}      | Bias table for job selection (e.g., prefer `COMPRESSION`, avoid `ANGEL`). |
| `daemon_penalty`  | struct                   | model   | Per‑model automation penalties: e.g., `quality:-5`, `time:+10%`.          |
| `daemon_unlocked` | bool                     | false   | Gate per unit (milestone, upgrade, or brand trait).                       |
| `daemon_priority` | i32                      | 0       | Higher = grabs jobs sooner than others when multiple Autos idle.          |

> Back‑compat: the former **global daemon** becomes an optional *Facility Template* (see below) that can apply defaults to new processors but does not force automation.

### UI/UX

- Processor list shows a small **daemon badge**: Off (∙), Assist (◧), Auto (⬤).
- Focused processor: hotkey **D** cycles Off → Assist → Auto. **Shift+D** opens a popover to edit `daemon_affinity` and `priority`.
- Jobs panel surfaces **suggestions** from Assist units (ghost rows) and allows one‑key assignment.

### Behavior

1. **Assist:**
   - Computes compatibility + simple score using speed, instruction match, power/heat budget, and `daemon_affinity`.
   - Renders top suggestion; player presses **A** to accept (or ignores).
2. **Auto:**
   - When idle, pulls the best‑scoring compatible job from the queue.
   - Applies `daemon_penalty` (stacking with global automation tunables if any).
   - Respects **cooling/overheat**: will refuse heavy jobs if it would push into Burnout thresholds unless configured otherwise.
3. **Contested Picks:**
   - If multiple Auto units target the same job, `daemon_priority` breaks ties; otherwise fastest ETA wins.
4. **Safety Rules:**
   - Won’t take jobs that require tags the unit lacks.
   - Optional toggle: “Honor Cooling Minimums.” If on and cooling below `requires_cooling_min`, only `GENERAL` tasks are considered.

### Facility Template (optional)

- A settings panel where you define a default `daemon_mode`, `daemon_affinity`, and `daemon_penalty` profile per **brand/model**.
- New purchases inherit the template; players can override per unit.

### Store Additions

- **Daemon Microcode** (per unit): unlocks `daemon_unlocked = true` and reduces `daemon_penalty` by a small, model‑specific amount.
- **Coordination Bus** (facility): +1 to `daemon_priority` for all units of a chosen brand for the current day.

### Persistence (save/load)

Add fields to the Processor instance schema:

```ron
(
  daemon_mode: Off,       // Off|Assist|Auto
  daemon_unlocked: false,
  daemon_affinity: { COMPRESSION: 0.8, ANGEL: -0.5 },
  daemon_priority: 0,
)
```

### Balancing Notes

- Start with the same automation tax used globally: `quality:-5`, `time:+10%` when `daemon_mode==Auto`.
- Some brands override defaults (e.g., Mnemosyne: smaller quality tax on COMPRESSION/STORAGE; Heretic: larger).
- Assist mode has **no penalty** since it’s still a human‑confirmed action.

### Simulation Hook (pseudocode)

```rust
fn maybe_auto_assign(proc: &Processor, jobs: &mut JobQueue) -> Option<JobId> {
  if !proc.daemon_unlocked || proc.daemon_mode != Auto || !proc.is_idle() { return None; }
  let candidates = jobs.compatible_with(&proc.instruction_set);
  if candidates.is_empty() { return None; }
  let scored = score_jobs(&proc, candidates, &proc.daemon_affinity);
  let best = break_ties_by_priority_eta(scored, proc.daemon_priority);
  if would_exceed_heat_budget(&proc, &best) && proc.honor_cooling_mins { return None; }
  Some(best.id)
}
```

### Acceptance Criteria (delta)

1. Per‑processor daemon states persist and render in UI; hotkeys work from processor focus.
2. Assist mode produces visible suggestions; Auto mode self‑assigns respecting compatibility, priority, and safety rules.
3. Automation penalties apply **per unit** when Auto is active.
4. Facility Template (if enabled) only seeds defaults—does not override per‑unit settings.


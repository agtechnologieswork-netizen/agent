## NiceGUI Widget System — Design & Integration Guide

This document explains how the widget system works in this branch, covering data models, services, rendering, user UI flows, and agent-driven creation paths. It serves as a practical guide for UI and SDK developers to extend and integrate the system safely.

### Goals
- Dynamic, data-driven widgets persisted in the database
- First-class support for user customization and agent code generation
- Safe data-source execution with clear UX patterns
- Extensible architecture for new widget types and data providers

---

## High-Level Architecture

```mermaid
graph TD
  subgraph UI[User UI]
    WPage[/"/widgets" page/]
    WMgr[WidgetManager]
    Grid[WidgetGrid]
  end

  subgraph Agent[Agent SDK]
    WTools[WidgetTools]
    WGen[WidgetGenerator]
  end

  subgraph Core[Core Services]
    WService[WidgetService]
    DSvc[DataSourceService]
    WRender[WidgetRenderer]
  end

  subgraph DB[(Database)]
    T1[widget]
    T2[widgettemplate]
    T3[userwidgetpreset]
  end

  subgraph External[External Sources]
    DBRX[Databricks Warehouse]
  end

WPage --> WMgr -->|CRUD, reorder| WService
WMgr --> Grid -->|render| WRender
WTools -->|create/update| WService
WGen -->|programmatic creation| WService
WRender -->|fetch data| DSvc
WService <--> DB
DSvc -->|SQL/Introspection| DB
DSvc -->|databricks_query| DBRX
```

---

## Key Modules

- `app/widget_models.py`
  - `Widget`, `WidgetTemplate`, `UserWidgetPreset` and enums `WidgetType`, `WidgetSize` (SQLModel + JSON fields)
- `app/widget_service.py`
  - CRUD, reorder, template/preset operations; default widget bootstrapping
- `app/data_source_service.py`
  - Table introspection, safe SQL execution, aggregations, Databricks execution wrapper
- `app/widget_renderer.py`
  - Per-type rendering logic; grid layout helper `WidgetGrid`
- `app/widget_ui.py`
  - `WidgetManager` user UI for add/edit/delete with dialogs; page `/widgets`
- `app/widget_tools.py`
  - Agent-friendly helpers to create data-driven widgets correctly
- `app/widget_generator.py`
  - Programmatic sample generators; enforces data-source-first philosophy
- `app/startup.py`
  - System initialization and default routes

---

## Data Model

### Enums
- `WidgetType`: `card | chart | table | metric | button | text | image | custom`
- `WidgetSize`: `small | medium | large | full` (mapped to 12-column grid)

### `Widget`
- Fields (selected):
  - `name: str`, `type: WidgetType`, `size: WidgetSize`, `position: int`, `page: str`
  - `config: dict` — visual/config properties (never store live data here)
  - `data_source: dict | None` — the data contract for fetching live data
  - `style: dict` — inline CSS style map (e.g., `{"background": "#fff"}`)
  - `is_visible: bool`, `is_editable: bool`

### `WidgetTemplate`
- Canonical components with default `config` and `style` users can instantiate

### `UserWidgetPreset`
- Per-user saved arrangement: widgets serialized and restored on demand

---

## Data Source Contract

All data-driven widgets must have a `data_source`. Supported shapes:

- `{"type": "table", "table": string, "columns": list[str], "limit": int, "order_by": string}`
- `{"type": "aggregation", "table": string, "aggregation": "count|sum|avg", "group_by": string, "value_column": string}`
- `{"type": "query", "query": string, "refresh_interval": int}`
- `{"type": "databricks_query", "query": string, "refresh_interval": int}`

Execution is handled by `DataSourceService.execute_widget_query(widget)` which returns shape-appropriate results:
- Metric: `{ value: number, rows?: list[dict] }`
- Chart/Table: `{ rows: list[dict] }` or `{ x: list, y: list, labels?: list }`

Safety and ergonomics:
- Blocks destructive SQL: rejects `DROP`/`DELETE`
- Validates table and column names for `table` sources
- Databricks queries include TTL cache and connection checks
- Backtick sanitization for broader SQL engine compatibility

Environment:
- App DB: `APP_DATABASE_URL` (PostgreSQL by default)
- Databricks: `DATABRICKS_HOST`, `DATABRICKS_TOKEN`
  - Optional cache tuning: `DATABRICKS_CACHE_TTL_SECONDS`, `DATABRICKS_CACHE_MAX_ENTRIES`

---

## Service Layer

`WidgetService` provides the single entry point for persistence:
- `get_widgets_for_page(page, user_id?) -> list[Widget]`
- `get_all_widgets() -> list[Widget]`
- `create_widget(name, type, page, size, config?, data_source?, style?) -> Widget`
- `update_widget(widget_id, **kwargs) -> Widget|None`
- `delete_widget(widget_id) -> bool`
- `reorder_widgets(page, widget_ids) -> bool`
- `get_widget_templates() -> list[WidgetTemplate]`
- `create_widget_from_template(template_id, page, name?) -> Widget|None`
- `save_user_preset(user_id, preset_name, page) -> UserWidgetPreset`
- `load_user_preset(preset_id, page) -> bool`
- `initialize_default_widgets(create_samples: bool = True) -> None`

Conventions enforced:
- For `metric|chart|table` without `data_source`, a safe default query is auto-added
- `position` is auto-assigned with stable ordering per page

---

## Rendering Layer

`WidgetRenderer.render_widget(widget, on_edit?, on_delete?)` dispatches by `WidgetType`:
- `TEXT`: Markdown/label
- `METRIC`: title, icon, numeric `value` (extracted from data source)
- `CHART`: Plotly `line|bar|pie`; resolves `x/y` from query rows or direct `x/y`
- `TABLE`: Auto-infers columns from first row; paginated via NiceGUI table
- `BUTTON`: `notify|navigate` actions
- `IMAGE`: url + optional caption
- `CARD`: title/subtitle/content HTML + actions
- `CUSTOM`: raw `html` and/or `javascript`

Size mapping (12-col grid via `WidgetGrid`):
- `small=3`, `medium=6`, `large=9`, `full=12`

Edit affordances:
- Hover action buttons (edit/delete) injected when callbacks provided

---

## User UI Flow (`WidgetManager`)

Routes:
- `/widgets` → complete management surface
- `/` integrates on startup as a fallback dashboard

Flows:
- Add Widget
  - Dialog collects: name, type, size
  - Data source selection is required: table, custom SQL, Databricks SQL
  - Type-specific config scaffold (e.g., chart type, labels)
  - Creates via `WidgetService.create_widget(...)`
- Edit Widget
  - Dialog shows name, size, current data source summary, JSON config editor
  - Saves via `WidgetService.update_widget(...)`
- Delete Widget
  - Confirmation dialog → `WidgetService.delete_widget(...)`
- Live View
  - `WidgetGrid.render(widgets, editable=True)` with per-widget actions

UX details:
- Edit mode toggle is supported in dashboard variants (e.g., `BIDashboardUI`)
- Dialog lifecycles are tracked (`dialog_open`) to avoid conflicting refreshes
- Notifications for success/errors are surfaced via NiceGUI `ui.notify`

---

## Agent Integration

Use `app/widget_tools.py` and `app/widget_generator.py` — never write raw DB code in agent flows:

Common helpers:
- `WidgetTools.create_metric_from_query(name, query, title?, icon?, page?, size?)`
- `WidgetTools.create_chart_from_table(name, table, x_column, y_column, chart_type?, title?, page?, size?, limit?)`
- `WidgetTools.create_table_from_query(name, query, title?, page?, size?, columns?)`
- `WidgetTools.create_widgets_for_table(table_name, page?)`
- `WidgetTools.create_dashboard_from_schema(page?)`

Programmatic samples:
- `WidgetGenerator.generate_sample_widgets()` creates data-backed defaults

Design rules enforced in helpers:
- Always set a `data_source` (metric/chart/table with static data is auto-converted)
- Sanitize and constrain SQL where possible; prefer `table`/`aggregation` sources when available
- Databricks path auto-selected when credentials are present

Example (Agent):
```python
from app.widget_tools import WidgetTools

WidgetTools.create_metric_from_query(
    name="Total Sales",
    query="SELECT SUM(amount) as value FROM sales",
    icon="trending_up",
)
```

---

## Initialization & Routing

- `app/startup.py`
  - `create_tables()` for SQLModel metadata
  - `initialize_widgets()` to hydrate default dashboard when empty
  - Registers `/` page that renders `WidgetManager` (fallback-safe)
- `app/bi_dashboard_ui.py`
  - Integrates a richer dashboard (`/bi-dashboard`) and embeds custom widget section using `WidgetManager` and `WidgetGrid`

---

## Extensibility Guidelines

Adding a new widget type (e.g., `map`):
1) Extend enums in `widget_models.py`:
   - Add `MAP = "map"` to `WidgetType`
2) Add rendering branch in `WidgetRenderer`:
   - Implement `_render_map_widget(widget)` and wire in `render_widget`
3) Provide default config in the UI form:
   - Update `WidgetManager.update_config_fields()` to scaffold type-specific inputs
4) Add SDK support if needed:
   - Add convenience creators in `WidgetTools`
5) Document data_source expectations:
   - Extend `DataSourceService.execute_widget_query` only if a new source type is required

Data providers:
- Prefer enhancing `table` and `aggregation` paths for performance and safety
- When introducing a new external provider, follow `databricks_query` shape and patterns (env validation, caching, error fallback)

Styling and layout:
- Use `widget.style` map for inline styles; avoid baking layout decisions into renderers
- Respect `WidgetSize` grid widths; keep row packing logic in `WidgetGrid`

---

## UX, Concurrency, and Slots

NiceGUI slot rules to avoid "slot stack empty" errors in async flows:
- Use container pattern for async updates: pass container references and wrap with `with container:` blocks
- Prefer `@ui.refreshable` for background-triggered UI changes; call `.refresh()` instead of creating UI off-context
- Clear containers before re-rendering sections

Event handlers / lambdas:
- Capture values safely when they may be `None`
- Keep handlers short; delegate to service or helper functions where possible

---

## Testing Guidance

- For UI tests, access single elements via `.elements.pop()`; convert to list for multiple
- Always await UI changes after interactions; avoid immediate assertions
- Prefer service layer unit tests for complex logic (e.g., `WidgetService`, `DataSourceService`)
- Verify data_source execution returns expected shapes (`value`, `rows`, `x/y`)

---

## Common Pitfalls & Gotchas

- Missing data_source on data-driven types
  - Mitigation: Helpers and `WidgetService.create_widget` add safe defaults
- Destructive SQL in custom queries
  - Mitigation: Basic guard in `execute_widget_query`
- Databricks credentials not set
  - Mitigation: Graceful fallback + informative hints in UI
- Static data placed into `config`
  - Mitigation: Generators/Tools auto-convert to queries when possible

---

## Future Enhancements (Suggested)

- Drag-and-drop reordering with persisted `position`
- Per-user visibility and RBAC for widgets
- Versioned widget configs with diff/rollback
- Schema-aware query builders in UI for safer custom SQL
- Server-side pagination for large tables

---

## Quick Reference

- Create metric (agent): see `WidgetTools.create_metric_from_query`
- Create chart (agent): see `WidgetTools.create_chart_from_table`
- Add widget (user): `/widgets` → Add Widget dialog, data source required
- Render widgets: `WidgetGrid.render(widgets, editable)` + `WidgetRenderer`
- Execute data: `DataSourceService.execute_widget_query(widget)`

---

## File Map

- `app/widget_models.py` — SQLModel entities and enums
- `app/widget_service.py` — CRUD and orchestration
- `app/data_source_service.py` — data introspection and query execution
- `app/widget_renderer.py` — per-type rendering, grid system
- `app/widget_ui.py` — user-facing management UI and routes
- `app/widget_tools.py` — agent SDK helpers
- `app/widget_generator.py` — programmatic sample creation
- `app/startup.py` — boot sequence and route registration



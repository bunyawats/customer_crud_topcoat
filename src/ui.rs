//! Shared `view!` components: the document shell, icons, form fields,
//! buttons and the error alert. Everything here is markup only - no
//! database access, no routing.
//!
//! Tailwind scans this file (see `static/input.css`), so keep class names as
//! string literals; anything assembled at runtime won't be picked up.

use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    router::{StatusCode, request::uri},
    view::{Child, View, class, component, view},
};

use crate::pagination::Page;

// --- Class lists ------------------------------------------------------------

pub const INPUT: &str = "mt-1 w-full rounded-lg border border-line bg-paper px-3 py-2 text-sm text-ink placeholder:text-muted focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand-tint";
pub const SELECT: &str = "mt-1 w-full rounded-lg border border-line bg-paper px-3 py-2 text-sm text-ink focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand-tint";
const BTN_PRIMARY: &str = "rounded-lg bg-brand px-4 py-2 text-sm font-medium text-white shadow-sm transition-colors hover:bg-brand-hover";
const BTN_GHOST: &str = "rounded-lg px-4 py-2 text-sm font-medium text-muted transition-colors hover:bg-paper hover:text-ink";
const BTN_DANGER: &str = "rounded-lg bg-danger px-4 py-2 text-sm font-medium text-white shadow-sm transition-colors hover:bg-danger/90";
const BTN_TOOLBAR_DANGER: &str = "inline-flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-xs font-medium text-danger transition-colors hover:bg-danger-tint disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent";
const BTN_PAGER: &str = "rounded-md border border-line px-2.5 py-1 font-medium text-ink transition-colors hover:bg-paper disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent";

// --- Document shell ---------------------------------------------------------

/// The top-level sections, used to highlight the sidebar nav and title the
/// page. Derived from the request path, so pages don't pass it around.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Customers,
    Products,
    Orders,
}

impl Section {
    pub fn from_path(path: &str) -> Option<Self> {
        let first = path.trim_start_matches('/').split('/').next()?;
        match first {
            "customers" => Some(Section::Customers),
            "products" => Some(Section::Products),
            "orders" => Some(Section::Orders),
            _ => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Section::Customers => "Customer Profiles",
            Section::Products => "Products",
            Section::Orders => "Orders",
        }
    }
}

/// The full HTML document: sidebar, alert region, the shared `<dialog>`
/// that create/edit forms load into, and the scripts. Used by the root
/// `#[layout]` for every page and by the error layer for full-page errors.
#[component]
pub async fn shell(
    cx: &Cx,
    #[default] title: Option<String>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let current = Section::from_path(uri(cx).path());
    let title = title
        .or_else(|| current.map(|s| s.title().to_string()))
        .unwrap_or_else(|| "Ledger".to_string());

    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>(title)</title>
                // htmx v4 is pre-GA, so npm's "latest" tag still points at
                // 2.x; "next" tracks the current v4 pre-release. Switch to
                // @4 once v4 ships.
                <script src="https://unpkg.com/htmx.org@next"></script>
                <link rel="preconnect" href="https://fonts.googleapis.com">
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="">
                <link
                    href="https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,500;0,9..144,600;1,9..144,500&family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans:wght@400;500;600&display=swap"
                    rel="stylesheet"
                >
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                topcoat::dev::script()
            </head>
            <body class="min-h-screen bg-paper font-sans text-ink antialiased">
                <div
                    id="alert-region"
                    class="fixed inset-x-0 top-4 z-50 flex justify-center px-4"
                ></div>

                <div class="mx-auto flex max-w-6xl flex-col sm:flex-row">
                    <aside
                        class="shrink-0 border-b border-line px-4 py-6 sm:w-56 sm:border-r sm:border-b-0 sm:py-10"
                    >
                        <p class="font-display text-lg font-semibold text-ink">
                            "Ledger"
                        </p>
                        <nav class="mt-4 flex gap-1 sm:mt-8 sm:flex-col">
                            nav_link(
                                href: "/customers",
                                label: "Customers",
                                active: current == Some(Section::Customers)
                            )
                            nav_link(
                                href: "/products",
                                label: "Products",
                                active: current == Some(Section::Products)
                            )
                            nav_link(
                                href: "/orders",
                                label: "Orders",
                                active: current == Some(Section::Orders)
                            )
                        </nav>
                    </aside>

                    <main class="min-w-0 flex-1 px-4 py-10 sm:py-14">(child)</main>
                </div>

                <dialog
                    id="crud-dialog"
                    class="dialog-anim m-auto w-[min(28rem,92vw)] rounded-xl border border-line bg-surface p-0 shadow-2xl backdrop:bg-ink/50"
                >
                    <button
                        type="button"
                        class="absolute right-3 top-3 rounded-full p-1.5 text-muted transition-colors hover:bg-paper hover:text-ink"
                        aria-label="Close"
                        onclick="crudDialog.close()"
                    >
                        icon(kind: Icon::X, size: 18)
                    </button>
                    <div id="dialog-alert-region" class="px-6 pt-6 empty:hidden"></div>
                    <div id="dialog-content" class="p-6"></div>
                </dialog>

                <script src=(asset!("../static/app.js")) defer=""></script>
            </body>
        </html>
    })
}

#[component]
async fn nav_link(href: &str, label: &str, active: bool) -> Result<impl View> {
    Ok(view! {
        <a
            href=(href)
            class=(class!("nav-link", "nav-link-active" if active))
            aria-current=(active.then_some("page"))
        >
            (label)
        </a>
    })
}

// --- Page furniture ---------------------------------------------------------

/// "Records / <title>" heading with the primary "Add …" button, which loads
/// the create form into the dialog.
#[component]
pub async fn page_header(title: &str, add_label: &str, new_url: &str) -> Result<impl View> {
    Ok(view! {
        <div
            class="flex items-baseline justify-between gap-4 border-b border-line pb-6"
        >
            <div>
                <p class="font-mono text-xs uppercase tracking-widest text-muted">
                    "Records"
                </p>
                <h1 class="font-display text-3xl font-semibold text-ink">(title)</h1>
            </div>
            <button
                type="button"
                class="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-brand px-4 py-2.5 text-sm font-medium text-white shadow-sm transition-colors hover:bg-brand-hover"
                hx-get=(new_url)
                hx-target="#dialog-content"
                hx-swap="innerHTML"
            >
                icon(kind: Icon::Plus)
                (add_label)
            </button>
        </div>
    })
}

/// The bordered table card. `child` is the `<thead>` + `<tbody id="rows-body">`.
#[component]
pub async fn table_card(#[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <div
            class="mt-6 overflow-hidden rounded-xl border border-line bg-surface shadow-sm"
        >
            <table class="w-full border-collapse text-sm">(child)</table>
        </div>
    })
}

/// A `<thead>` row from column labels. An empty label renders a
/// screen-reader-only "Actions" header. `child` fills the leading selection
/// column (the "select all on this page" checkbox).
#[component]
pub async fn table_head(
    columns: &'static [&'static str],
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <thead>
            <tr
                class="border-b border-line text-left text-xs uppercase tracking-wide text-muted"
            >
                <th class="w-10 py-3 pl-4">(child)</th>
                for column in columns {
                    <th class="px-4 py-3 font-medium">
                        if column.is_empty() {
                            <span class="sr-only">"Actions"</span>
                        } else {
                            (*column)
                        }
                    </th>
                }
            </tr>
        </thead>
    })
}

/// The "nothing here yet" row shown when a table has no records.
#[component]
pub async fn empty_rows(colspan: u32, title: &str, hint: &str) -> Result<impl View> {
    Ok(view! {
        <tr>
            <td colspan=(colspan) class="px-4 py-12 text-center">
                <p class="font-display text-lg italic text-muted">(title)</p>
                <p class="mt-1 text-sm text-muted">(hint)</p>
            </td>
        </tr>
    })
}

/// Edit (opens the form in the dialog) and delete buttons for a table row.
/// Delete re-renders the whole `#<list_id>` region for the page the user is
/// on (`hx-include` sends its hidden `page` input), so the pager's counts
/// stay right and the page refills from the next one.
#[component]
pub async fn row_actions(
    label: &str,
    list_id: &str,
    edit_url: &str,
    delete_url: &str,
    confirm: &str,
) -> Result<impl View> {
    let list_target = format!("#{list_id}");
    let state_selector = format!("#{list_id}-state");
    Ok(view! {
        <td class="px-2 py-3 text-right">
            <div class="flex justify-end gap-1">
                <button
                    type="button"
                    class="rounded-md p-1.5 text-muted transition-colors hover:bg-brand-tint hover:text-brand"
                    aria-label=(format!("Edit {label}"))
                    hx-get=(edit_url)
                    hx-target="#dialog-content"
                    hx-swap="innerHTML"
                >
                    icon(kind: Icon::Edit)
                </button>
                <button
                    type="button"
                    class="rounded-md p-1.5 text-muted transition-colors hover:bg-danger-tint hover:text-danger"
                    aria-label=(format!("Delete {label}"))
                    hx-delete=(delete_url)
                    hx-target=(&list_target)
                    hx-swap="outerHTML"
                    hx-include=(&state_selector)
                    hx-confirm=(confirm)
                >
                    icon(kind: Icon::Trash)
                </button>
            </div>
        </td>
    })
}

// --- Selection --------------------------------------------------------------

const CHECKBOX: &str = "size-4 cursor-pointer rounded border-line accent-brand align-middle";

/// The leading cell of a selectable row. Ticking it posts to `select_url`
/// with `hx-swap="none"`: the box is already right, the post only records it
/// server-side, and the response's out-of-band toolbar updates the count.
/// `hx-include` adds the list's `page`/`query_id`, so the row itself doesn't
/// need to know which page it's on.
#[component]
pub async fn row_checkbox(
    id: i64,
    label: &str,
    checked: bool,
    select_url: &str,
    list_id: &str,
) -> Result<impl View> {
    let vals = format!(r#"js:{{"ids": "{id}", "checked": event.target.checked}}"#);
    let state_selector = format!("#{list_id}-state");
    Ok(view! {
        <td
            class="w-10 border-l-2 border-l-transparent py-3 pl-[14px] transition-colors group-hover:border-l-brand"
        >
            <input
                type="checkbox"
                class=(CHECKBOX)
                aria-label=(format!("Select {label}"))
                checked=(checked)
                hx-post=(select_url)
                hx-swap="none"
                hx-include=(&state_selector)
                hx-vals=(&vals)
            >
        </td>
    })
}

/// "Select all on this page": posts every id on the page as one
/// comma-joined string (an array in `hx-vals` would arrive mangled, see the
/// htmx4 skill) and re-renders the list so the row boxes follow.
#[component]
pub async fn select_all_checkbox(
    ids: &[i64],
    checked: bool,
    select_url: &str,
    list_id: &str,
) -> Result<impl View> {
    let joined = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let vals = format!(r#"js:{{"ids": "{joined}", "checked": event.target.checked}}"#);
    let target = format!("#{list_id}");
    let state_selector = format!("#{list_id}-state");
    Ok(view! {
        <input
            type="checkbox"
            class=(CHECKBOX)
            aria-label="Select all on this page"
            checked=(checked)
            disabled=(ids.is_empty())
            hx-post=(select_url)
            hx-target=(&target)
            hx-swap="outerHTML"
            hx-include=(&state_selector)
            hx-vals=(&vals)
        >
    })
}

/// The selection toolbar above a list: how many rows are selected across
/// all pages, and "Delete selected", which loads the confirm dialog from
/// `confirm_url` (the list's `page` rides along so the dialog can send it on
/// to the delete). Rendered inline on page load, and sent out-of-band
/// (`oob`) by every response that changes the selection.
#[component]
pub async fn bulk_toolbar(
    toolbar_id: &str,
    count: usize,
    confirm_url: &str,
    list_id: &str,
    #[default] oob: bool,
) -> Result<impl View> {
    let state_selector = format!("#{list_id}-state");
    Ok(view! {
        <div
            id=(toolbar_id)
            hx-swap-oob=(oob.then_some("true"))
            class="mt-6 -mb-3 flex min-h-9 items-center justify-between gap-4 text-sm"
        >
            <span
                class=(class!(
                    "font-mono text-xs",
                    "text-muted" if count == 0,
                    "text-ink" if count > 0,
                ))
            >
                if count == 0 {
                    "No rows selected"
                } else {
                    (count)
                    " selected"
                }
            </span>
            <button
                type="button"
                class=(BTN_TOOLBAR_DANGER)
                disabled=(count == 0)
                hx-post=(confirm_url)
                hx-target="#dialog-content"
                hx-swap="innerHTML"
                hx-include=(&state_selector)
            >
                icon(kind: Icon::Trash, size: 14)
                "Delete selected"
            </button>
        </div>
    })
}

/// One row as the bulk confirm dialog lists it: fetched fresh from the
/// database, never from text the client sent.
pub struct BulkItem {
    pub id: i64,
    pub label: String,
    pub detail: String,
}

/// A row the bulk action couldn't delete, and why.
pub struct BulkFailure {
    pub label: String,
    pub error: String,
}

/// "1 customer" / "3 customers".
fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The confirm step before a bulk delete, loaded into the shared dialog.
/// It names what will be deleted; the delete itself re-reads the selection
/// server-side, so the form carries no ids, only the `page` to re-render.
#[component]
pub async fn bulk_confirm_dialog(
    one: &str,
    many: &str,
    items: &[BulkItem],
    error: Option<&str>,
    execute_url: &str,
    page: i64,
) -> Result<impl View> {
    let title = if items.is_empty() {
        format!("Delete {many}")
    } else {
        format!("Delete {}?", counted(items.len(), one, many))
    };
    let submit = format!("Delete {}", counted(items.len(), one, many));
    let can_delete = error.is_none() && !items.is_empty();
    Ok(view! {
        <form
            id="bulk-delete-form"
            hx-post=(execute_url)
            hx-target="#dialog-content"
            hx-swap="innerHTML"
        >
            <input type="hidden" name="page" value=(page)>
            form_header(
                title: &title,
                subtitle: "Rows that can't be deleted are skipped and listed afterwards."
            )
            if let Some(error) = error {
                <p
                    class="mt-4 rounded-lg border-l-4 border-danger bg-danger-tint px-4 py-3 text-sm text-ink"
                    role="alert"
                >
                    (error)
                </p>
            } else if items.is_empty() {
                <p class="mt-4 text-sm text-muted">"Nothing is selected."</p>
            } else {
                <ul
                    class="mt-4 max-h-64 divide-y divide-line overflow-y-auto rounded-lg border border-line text-sm"
                >
                    for item in items {
                        <li class="flex items-baseline justify-between gap-4 px-3 py-2">
                            <span class="font-medium text-ink">(&item.label)</span>
                            <span class="truncate font-mono text-xs text-muted">
                                (&item.detail)
                            </span>
                        </li>
                    }
                </ul>
            }
            <div class="mt-6 flex justify-end gap-2">
                <button type="button" class=(BTN_GHOST) onclick="crudDialog.close()">
                    "Cancel"
                </button>
                if can_delete {
                    <button type="submit" class=(BTN_DANGER)>(&submit)</button>
                }
            </div>
        </form>
    })
}

/// What a bulk delete did, shown in the dialog in place of the confirm form.
#[component]
pub async fn bulk_result_dialog(
    one: &str,
    many: &str,
    deleted: usize,
    failures: &[BulkFailure],
) -> Result<impl View> {
    let title = format!("Deleted {}", counted(deleted, one, many));
    let subtitle = if failures.is_empty() {
        "Every selected row was deleted.".to_string()
    } else {
        format!(
            "{} could not be deleted:",
            counted(failures.len(), one, many)
        )
    };
    Ok(view! {
        form_header(title: &title, subtitle: &subtitle)
        if !failures.is_empty() {
            <ul
                class="mt-4 max-h-64 divide-y divide-line overflow-y-auto rounded-lg border border-danger/40 text-sm"
            >
                for failure in failures {
                    <li class="px-3 py-2">
                        <p class="font-medium text-ink">(&failure.label)</p>
                        <p class="text-xs text-danger">(&failure.error)</p>
                    </li>
                }
            </ul>
        }
        <div class="mt-6 flex justify-end">
            <button type="button" class=(BTN_PRIMARY) onclick="crudDialog.close()">
                "Close"
            </button>
        </div>
    })
}

// --- Pagination -------------------------------------------------------------

/// What the pager footer shows, copied out of a [`Page`] so the list
/// component can hand its rows to the row components separately.
pub struct PagerInfo {
    pub page: i64,
    pub last_page: i64,
    pub first: i64,
    pub last: i64,
    pub total: i64,
    pub has_prev: bool,
    pub has_next: bool,
    pub query_id: String,
}

impl<T> From<&Page<T>> for PagerInfo {
    fn from(page: &Page<T>) -> Self {
        PagerInfo {
            page: page.page,
            last_page: page.last_page(),
            first: page.first_item_number(),
            last: page.last_item_number(),
            total: page.total,
            has_prev: page.has_prev(),
            has_next: page.has_next(),
            query_id: page.query_id.clone(),
        }
    }
}

/// Hidden `page`/`query_id` inputs for the list currently shown, at
/// `#<list_id>-state`. Row and toolbar controls `hx-include` it, so rows
/// themselves don't need to know which page they're on.
#[component]
pub async fn list_state(list_id: &str, page: i64, query_id: &str) -> Result<impl View> {
    Ok(view! {
        <div id=(format!("{list_id}-state")) hidden="">
            <input type="hidden" name="page" value=(page)>
            <input type="hidden" name="query_id" value=(query_id)>
        </div>
    })
}

/// The table footer: "Showing a–b of N" and Prev/Next, which post `page` and
/// the cached-count `query_id` to `url` and replace `#<list_id>`.
#[component]
pub async fn pager(colspan: u32, url: &str, list_id: &str, info: &PagerInfo) -> Result<impl View> {
    let target = format!("#{list_id}");
    let prev_vals = page_vals(info.page - 1, &info.query_id);
    let next_vals = page_vals(info.page + 1, &info.query_id);
    Ok(view! {
        <tfoot>
            <tr class="border-t border-line">
                <td colspan=(colspan) class="px-4 py-3">
                    <div
                        class="flex items-center justify-between gap-4 text-xs text-muted"
                    >
                        <span class="font-mono tabular-nums">
                            if info.total > 0 {
                                "Showing "
                                (info.first)
                                "–"
                                (info.last)
                                " of "
                                (info.total)
                            } else {
                                "No records"
                            }
                        </span>
                        <div class="flex items-center gap-2">
                            <span class="font-mono tabular-nums">
                                "Page "
                                (info.page)
                                " of "
                                (info.last_page)
                            </span>
                            <button
                                type="button"
                                class=(BTN_PAGER)
                                disabled=(!info.has_prev)
                                hx-post=(url)
                                hx-target=(&target)
                                hx-swap="outerHTML"
                                hx-vals=(&prev_vals)
                            >
                                "Prev"
                            </button>
                            <button
                                type="button"
                                class=(BTN_PAGER)
                                disabled=(!info.has_next)
                                hx-post=(url)
                                hx-target=(&target)
                                hx-swap="outerHTML"
                                hx-vals=(&next_vals)
                            >
                                "Next"
                            </button>
                        </div>
                    </div>
                </td>
            </tr>
        </tfoot>
    })
}

fn page_vals(page: i64, query_id: &str) -> String {
    serde_json::json!({ "page": page, "query_id": query_id }).to_string()
}

// --- Forms ------------------------------------------------------------------

/// Heading block at the top of a dialog form.
#[component]
pub async fn form_header(title: &str, subtitle: &str) -> Result<impl View> {
    Ok(view! {
        <h2 class="font-display text-xl font-semibold text-ink">(title)</h2>
        <p class="mt-1 text-sm text-muted">(subtitle)</p>
    })
}

/// A labelled form control: `child` is the `<input>`/`<select>`/`<textarea>`.
#[component]
pub async fn field(
    label: &str,
    #[default] class: Option<&'static str>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <label class=(class!("block", class))>
            <span class="text-xs font-medium uppercase tracking-wide text-muted">
                (label)
            </span>
            (child)
        </label>
    })
}

/// Cancel (closes the dialog) and submit buttons at the bottom of a form.
#[component]
pub async fn form_actions(submit_label: &str) -> Result<impl View> {
    Ok(view! {
        <div class="mt-6 flex justify-end gap-2">
            <button type="button" class=(BTN_GHOST) onclick="crudDialog.close()">
                "Cancel"
            </button>
            <button type="submit" class=(BTN_PRIMARY)>(submit_label)</button>
        </div>
    })
}

// --- Errors -----------------------------------------------------------------

/// The error alert, delivered as two out-of-band swaps: one into the page's
/// top `#alert-region`, one into the dialog's `#dialog-alert-region` (so an
/// error from a form submit is visible while the dialog is open). Sent with
/// `HX-Reswap: none`, so the element that made the request is untouched.
/// `static/app.js` auto-dismisses it.
#[component]
pub async fn alert(message: &str) -> Result<impl View> {
    Ok(view! {
        <div
            id="alert-region"
            hx-swap-oob="true"
            class="fixed inset-x-0 top-4 z-50 flex justify-center px-4"
        >
            <div
                class="flex w-full max-w-md items-start gap-2.5 rounded-lg border-l-4 border-danger bg-danger-tint px-4 py-3 text-sm text-ink shadow-lg transition-opacity duration-500 motion-reduce:transition-none"
                role="alert"
                data-auto-dismiss=""
            >
                icon(
                    kind: Icon::Alert,
                    size: 18,
                    class: Some("mt-0.5 shrink-0 text-danger")
                )
                <span>(message)</span>
            </div>
        </div>
        <div id="dialog-alert-region" hx-swap-oob="true" class="px-6 pt-6 empty:hidden">
            <div
                class="flex items-start gap-2.5 rounded-lg border-l-4 border-danger bg-danger-tint px-4 py-3 text-sm text-ink transition-opacity duration-500 motion-reduce:transition-none"
                role="alert"
                data-auto-dismiss=""
            >
                icon(
                    kind: Icon::Alert,
                    size: 18,
                    class: Some("mt-0.5 shrink-0 text-danger")
                )
                <span>(message)</span>
            </div>
        </div>
    })
}

/// Body of the full-page error, rendered inside `shell`.
#[component]
pub async fn error_page(status: StatusCode, message: &str) -> Result<impl View> {
    Ok(view! {
        <div class="mx-auto max-w-md text-center">
            <p class="font-mono text-xs uppercase tracking-widest text-muted">
                "Error "
                (status.as_u16())
            </p>
            <p class="mt-3 font-display text-xl text-ink">(message)</p>
            <a
                href="/customers"
                class="mt-6 inline-flex items-center gap-1.5 rounded-lg bg-brand px-4 py-2.5 text-sm font-medium text-white shadow-sm transition-colors hover:bg-brand-hover"
            >
                "Back to customers"
            </a>
        </div>
    })
}

// --- Icons ------------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum Icon {
    Edit,
    Trash,
    Plus,
    X,
    Alert,
}

/// Feather-style stroke icons, inlined so they inherit `currentColor`.
#[component]
pub async fn icon(
    kind: Icon,
    #[default(16)] size: u32,
    #[default] class: Option<&'static str>,
) -> Result<impl View> {
    Ok(view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width=(size)
            height=(size)
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class=(class)
            aria-hidden="true"
        >
            match kind {
                Icon::Edit => {
                    <path
                        d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
                    ></path>
                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                }
                Icon::Trash => {
                    <polyline points="3 6 5 6 21 6"></polyline>
                    <path
                        d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                    ></path>
                }
                Icon::Plus => {
                    <line x1="12" y1="5" x2="12" y2="19"></line>
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                }
                Icon::X => {
                    <line x1="18" y1="6" x2="6" y2="18"></line>
                    <line x1="6" y1="6" x2="18" y2="18"></line>
                }
                Icon::Alert => {
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="12" y1="8" x2="12" y2="12"></line>
                    <line x1="12" y1="16" x2="12.01" y2="16"></line>
                }
            }
        </svg>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_comes_from_first_path_segment() {
        assert!(Section::from_path("/customers") == Some(Section::Customers));
        assert!(Section::from_path("/products/7/edit") == Some(Section::Products));
        assert!(Section::from_path("/orders/") == Some(Section::Orders));
        assert!(Section::from_path("/").is_none());
        assert!(Section::from_path("/nope").is_none());
    }
}

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

// --- Class lists ------------------------------------------------------------

pub const INPUT: &str = "mt-1 w-full rounded-lg border border-line bg-paper px-3 py-2 text-sm text-ink placeholder:text-muted focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand-tint";
pub const SELECT: &str = "mt-1 w-full rounded-lg border border-line bg-paper px-3 py-2 text-sm text-ink focus:border-brand focus:bg-surface focus:outline-none focus:ring-2 focus:ring-brand-tint";
const BTN_PRIMARY: &str = "rounded-lg bg-brand px-4 py-2 text-sm font-medium text-white shadow-sm transition-colors hover:bg-brand-hover";
const BTN_GHOST: &str = "rounded-lg px-4 py-2 text-sm font-medium text-muted transition-colors hover:bg-paper hover:text-ink";

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
/// screen-reader-only "Actions" header.
#[component]
pub async fn table_head(columns: &'static [&'static str]) -> Result<impl View> {
    Ok(view! {
        <thead>
            <tr
                class="border-b border-line text-left text-xs uppercase tracking-wide text-muted"
            >
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

/// Edit (opens the form in the dialog) and delete (removes the row) buttons
/// for a table row. The `#<row_id>` element is the delete swap target.
#[component]
pub async fn row_actions(
    label: &str,
    row_id: &str,
    edit_url: &str,
    delete_url: &str,
    confirm: &str,
) -> Result<impl View> {
    let row_target = format!("#{row_id}");
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
                    hx-target=(row_target)
                    hx-swap="outerHTML"
                    hx-confirm=(confirm)
                >
                    icon(kind: Icon::Trash)
                </button>
            </div>
        </td>
    })
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

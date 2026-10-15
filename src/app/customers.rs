//! `/customers`: the list page, and `POST` to create.
//!
//! Also home to the customer `view!` components (row, rows, form), which the
//! child modules reuse for their fragment responses.

pub(crate) mod bulk_delete;
pub(crate) mod customer_id;
pub(crate) mod new;
pub(crate) mod page;
pub(crate) mod selection;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, page, route},
    view::{View, ViewExt, ViewHandle, component, view},
};

use std::collections::HashSet;

use super::{clear_selection, pool, selected_ids};
use crate::{
    customer::{
        models::{Customer, CustomerForm},
        services as service,
    },
    error::AppError,
    pagination::{Page, PageRequest},
    selection::models::Resource,
    ui::{self, ACTIONS_COLUMN, Column, INPUT, PagerInfo},
};
use customer_id::CustomerId;

/// Id of the list region; `#<LIST_ID>-state` holds its hidden page inputs.
pub(crate) const LIST_ID: &str = "customer-list";
/// Which stored selection this list's checkboxes read and write.
pub(crate) const RESOURCE: Resource = Resource::Customers;
/// Id of the selection toolbar above the list (sent out-of-band on changes).
pub(crate) const TOOLBAR_ID: &str = "customer-toolbar";
const LIST_TARGET: &str = "#customer-list";
/// Columns after the selection box. Fixed widths for the short, regular
/// data; name and email share the rest.
const COLUMNS: &[Column] = &[
    Column {
        label: "Name",
        width: "",
    },
    Column {
        label: "Email",
        width: "",
    },
    Column {
        label: "Phone",
        width: "w-40",
    },
    Column {
        label: "Member since",
        width: "w-36",
    },
    ACTIONS_COLUMN,
];

/// Flat, owned view model for a single table row: the components render
/// this rather than the domain `Customer`, with dates already formatted.
pub(crate) struct CustomerRow {
    id: i64,
    first_name: String,
    last_name: String,
    email: String,
    phone: Option<String>,
    member_since: String,
    last_updated: String,
}

impl From<Customer> for CustomerRow {
    fn from(customer: Customer) -> Self {
        CustomerRow {
            id: customer.id,
            first_name: customer.first_name,
            last_name: customer.last_name,
            member_since: customer.created_at.format("%Y-%m-%d").to_string(),
            last_updated: customer.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            email: customer.email,
            phone: customer.phone,
        }
    }
}

/// `GET /customers`
#[page]
pub(crate) async fn index(cx: &Cx) -> Result<impl View> {
    clear_selection(cx, RESOURCE).await?;
    let selected = HashSet::new();
    let listing = service::list_page(pool(cx), PageRequest::first())
        .await?
        .map(CustomerRow::from);
    let new_url = href!(new::new_form).resolve(cx);

    Ok(view! {
        ui::page_header(
            title: "Customer Profiles",
            add_label: "Add customer",
            new_url: &new_url
        )
        customer_toolbar(count: 0)
        customer_list(listing: listing, selected: &selected)
    })
}

/// `POST /customers` - creates the customer, then re-renders the list on the
/// page that holds it (so the new row is visible in its sorted position).
#[route(POST)]
pub(crate) async fn create(cx: &Cx, Form(form): Form<CustomerForm>) -> Result<ViewHandle> {
    let id = service::create(pool(cx), form).await?.id;
    let page = service::page_of(pool(cx), id).await?;
    let listing = service::list_page(pool(cx), PageRequest::new(page, ""))
        .await?
        .map(CustomerRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! { cx => customer_list(listing: listing, selected: &selected) }
        .single()
        .await
}

/// The `#customer-list` region: hidden list state, the table and its pager.
/// Prev/Next, create and delete all replace it whole (`outerHTML`).
#[component]
pub(crate) async fn customer_list(
    cx: &Cx,
    listing: Page<CustomerRow>,
    selected: &HashSet<i64>,
    #[default] oob: bool,
) -> Result<impl View> {
    let page_url = href!(page::show_page).resolve(cx);
    let select_url = href!(selection::update_selection).resolve(cx);
    let page_ids: Vec<i64> = listing.items.iter().map(|row| row.id).collect();
    let all_selected = !page_ids.is_empty() && page_ids.iter().all(|id| selected.contains(id));
    let pager = PagerInfo::from(&listing);
    let customers = listing.items;
    Ok(view! {
        <div id=(LIST_ID) hx-swap-oob=(oob.then_some("true"))>
            ui::list_state(
                list_id: LIST_ID,
                page: pager.page,
                query_id: &pager.query_id
            )
            ui::table_card(
                ui::table_head(
                    columns: COLUMNS,
                    ui::select_all_checkbox(
                        ids: &page_ids,
                        checked: all_selected,
                        select_url: &select_url,
                        list_id: LIST_ID
                    )
                )
                <tbody id="rows-body">
                    customer_rows(customers: customers, selected: selected)
                </tbody>
                ui::pager(colspan: 6, url: &page_url, list_id: LIST_ID, info: &pager)
            )
        </div>
    })
}

/// This list's selection toolbar (see [`ui::bulk_toolbar`]).
#[component]
pub(crate) async fn customer_toolbar(
    cx: &Cx,
    count: usize,
    #[default] oob: bool,
) -> Result<impl View> {
    let confirm_url = href!(bulk_delete::confirm::confirm_bulk_delete).resolve(cx);
    Ok(view! {
        ui::bulk_toolbar(
            toolbar_id: TOOLBAR_ID,
            count: count,
            confirm_url: &confirm_url,
            list_id: LIST_ID,
            oob: oob
        )
    })
}

/// Fresh display fields for the selected ids, for the bulk dialogs. Ids
/// that no longer exist (deleted elsewhere) are dropped.
pub(crate) async fn bulk_items(cx: &Cx, ids: &[i64]) -> Result<Vec<ui::BulkItem>> {
    let mut items = Vec::with_capacity(ids.len());
    for &id in ids {
        match service::get(pool(cx), id).await {
            Ok(customer) => items.push(ui::BulkItem {
                id,
                label: format!("{} {}", customer.first_name, customer.last_name),
                detail: customer.email,
            }),
            Err(AppError::NotFound(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(items)
}

#[component]
pub(crate) async fn customer_rows(
    customers: Vec<CustomerRow>,
    selected: &HashSet<i64>,
) -> Result<impl View> {
    Ok(view! {
        if customers.is_empty() {
            ui::empty_rows(
                colspan: 6,
                title: "The ledger is empty.",
                hint: "Add your first customer to get started."
            )
        } else {
            #[key(customer.id)]
            for customer in customers {
                customer_row(
                    customer: &customer,
                    selected: selected.contains(&customer.id)
                )
            }
        }
    })
}

/// One read-only table row. Its id, `customer-row-<id>`, is the target that
/// the edit form and the delete button swap.
#[component]
pub(crate) async fn customer_row(
    cx: &Cx,
    customer: &CustomerRow,
    selected: bool,
) -> Result<impl View> {
    let select_url = href!(selection::update_selection).resolve(cx);
    let row_id = format!("customer-row-{}", customer.id);
    let name = format!("{} {}", customer.first_name, customer.last_name);
    let edit_url = href!(customer_id::edit::edit_form, CustomerId(customer.id)).resolve(cx);
    let delete_url = href!(customer_id::delete, CustomerId(customer.id)).resolve(cx);

    Ok(view! {
        <tr
            id=(&row_id)
            class="group border-b border-line text-ink last:border-0 transition-colors hover:bg-paper has-checked:bg-brand-tint/40"
            title=(format!("Last updated {}", customer.last_updated))
        >
            ui::row_checkbox(
                id: customer.id,
                label: &name,
                checked: selected,
                select_url: &select_url,
                list_id: LIST_ID
            )
            <td class="truncate px-4 py-3 font-medium" title=(&name)>(&name)</td>
            <td class="truncate px-4 py-3 text-muted" title=(&customer.email)>
                (&customer.email)
            </td>
            <td class="truncate px-4 py-3 font-mono text-xs text-muted">
                (customer.phone.as_deref().unwrap_or("—"))
            </td>
            <td class="px-4 py-3 text-right font-mono text-xs tabular-nums text-muted">
                (&customer.member_since)
            </td>
            ui::row_actions(
                label: &name,
                list_id: LIST_ID,
                edit_url: &edit_url,
                delete_url: &delete_url,
                confirm: "Delete this customer?"
            )
        </tr>
    })
}

/// The create form (`existing: None`) or the edit form for one customer.
/// Create posts to `/customers` and refreshes all rows; edit puts to
/// `/customers/{id}` and swaps just that row.
#[component]
pub(crate) async fn customer_form(
    cx: &Cx,
    #[default] existing: Option<&CustomerRow>,
) -> Result<impl View> {
    let (form_id, title, subtitle, submit) = match existing {
        None => (
            "create-customer-form",
            "Add customer",
            "Create a new record in the ledger.",
            "Add customer",
        ),
        Some(_) => (
            "edit-customer-form",
            "Edit customer",
            "Update this record in the ledger.",
            "Save changes",
        ),
    };
    let target = match existing {
        None => href!(create).resolve(cx),
        Some(customer) => href!(customer_id::update, CustomerId(customer.id)).resolve(cx),
    };

    Ok(view! {
        <form
            id=(form_id)
            match existing {
                None => {
                    hx-post=(&target)
                    hx-target=(LIST_TARGET)
                    hx-swap="outerHTML"
                }
                Some(customer) => {
                    hx-put=(&target)
                    hx-target=(format!("#customer-row-{}", customer.id))
                    hx-swap="outerHTML"
                }
            }
        >
            ui::form_header(title: title, subtitle: subtitle)

            <div class="mt-5 space-y-4">
                ui::field(
                    label: "First name",
                    <input
                        type="text"
                        name="first_name"
                        value=(existing.map(|c| c.first_name.as_str()))
                        required=""
                        autofocus=""
                        class=(INPUT)
                    >
                )
                ui::field(
                    label: "Last name",
                    <input
                        type="text"
                        name="last_name"
                        value=(existing.map(|c| c.last_name.as_str()))
                        required=""
                        class=(INPUT)
                    >
                )
                ui::field(
                    label: "Email",
                    <input
                        type="email"
                        name="email"
                        value=(existing.map(|c| c.email.as_str()))
                        required=""
                        class=(INPUT)
                    >
                )
                ui::field(
                    label: "Phone",
                    <input
                        type="tel"
                        name="phone"
                        value=(existing.and_then(|c| c.phone.as_deref()))
                        class=(INPUT)
                    >
                )
            </div>

            ui::form_actions(submit_label: submit)
        </form>
    })
}

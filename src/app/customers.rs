//! `/customers`: the list page, and `POST` to create.
//!
//! Also home to the customer `view!` components (row, rows, form), which the
//! child modules reuse for their fragment responses.

pub(crate) mod customer_id;
pub(crate) mod new;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, page, route},
    view::{View, ViewExt, ViewHandle, component, view},
};

use super::pool;
use crate::{
    customer::{
        models::{Customer, CustomerForm},
        services as service,
    },
    ui::{self, INPUT},
};
use customer_id::CustomerId;

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

fn to_rows(customers: Vec<Customer>) -> Vec<CustomerRow> {
    customers.into_iter().map(CustomerRow::from).collect()
}

/// `GET /customers`
#[page]
pub(crate) async fn index(cx: &Cx) -> Result<impl View> {
    let customers = to_rows(service::list(pool(cx)).await?);
    let new_url = href!(new::new_form).resolve(cx);

    Ok(view! {
        ui::page_header(
            title: "Customer Profiles",
            add_label: "Add customer",
            new_url: &new_url
        )
        ui::table_card(
            ui::table_head(columns: &["Name", "Email", "Phone", "Member since", ""])
            <tbody id="rows-body">customer_rows(customers: customers)</tbody>
        )
    })
}

/// `POST /customers` - creates the customer, then re-renders every row into
/// `#rows-body` (so the new one lands in sorted position).
#[route(POST)]
pub(crate) async fn create(cx: &Cx, Form(form): Form<CustomerForm>) -> Result<ViewHandle> {
    service::create(pool(cx), form).await?;
    let customers = to_rows(service::list(pool(cx)).await?);
    view! { cx => customer_rows(customers: customers) }
        .single()
        .await
}

#[component]
pub(crate) async fn customer_rows(customers: Vec<CustomerRow>) -> Result<impl View> {
    Ok(view! {
        if customers.is_empty() {
            ui::empty_rows(
                colspan: 5,
                title: "The ledger is empty.",
                hint: "Add your first customer to get started."
            )
        } else {
            #[key(customer.id)]
            for customer in customers {
                customer_row(customer: &customer)
            }
        }
    })
}

/// One read-only table row. Its id, `customer-row-<id>`, is the target that
/// the edit form and the delete button swap.
#[component]
pub(crate) async fn customer_row(cx: &Cx, customer: &CustomerRow) -> Result<impl View> {
    let row_id = format!("customer-row-{}", customer.id);
    let name = format!("{} {}", customer.first_name, customer.last_name);
    let edit_url = href!(customer_id::edit::edit_form, CustomerId(customer.id)).resolve(cx);
    let delete_url = href!(customer_id::delete, CustomerId(customer.id)).resolve(cx);

    Ok(view! {
        <tr
            id=(&row_id)
            class="group border-b border-line text-ink last:border-0 transition-colors hover:bg-paper"
            title=(format!("Last updated {}", customer.last_updated))
        >
            <td
                class="border-l-2 border-l-transparent py-3 pr-4 pl-[14px] font-medium transition-colors group-hover:border-l-brand"
            >
                (&name)
            </td>
            <td class="px-4 py-3 text-muted">(&customer.email)</td>
            <td class="px-4 py-3 font-mono text-xs text-muted">
                (customer.phone.as_deref().unwrap_or("—"))
            </td>
            <td class="px-4 py-3 text-right font-mono text-xs tabular-nums text-muted">
                (&customer.member_since)
            </td>
            ui::row_actions(
                label: &name,
                row_id: &row_id,
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
                    hx-target="#rows-body"
                    hx-swap="innerHTML"
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

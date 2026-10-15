// NOTE ON HTMX v4 EVENTS: v4 renamed its lifecycle events to a colon-
// namespaced scheme (`htmx:after:swap`, `htmx:after:request`, ...) instead
// of v2's camelCase (`htmx:afterSwap`, `htmx:afterRequest`). The event
// `detail` shape changed too: v4 passes `{ ctx }` (the whole request
// context - `ctx.target`, `ctx.response.status`, etc.) instead of v2's flat
// `{ successful, target, ... }`. Re-check both when htmx v4 reaches GA, in
// case the shape changes again before then.

// Dialog lifecycle: shared by every resource's "create" and "edit" forms
// (customers, products, ...) - opening/closing the <dialog> itself has
// nothing to do with which form is inside it. Form-specific behavior lives
// in the per-resource handlers below.
const crudDialog = (() => {
    const dialog = document.getElementById("crud-dialog");
    const content = document.getElementById("dialog-content");

    function open() {
        if (!dialog.open) {
            dialog.showModal();
        }
    }

    function close() {
        dialog.close();
        content.replaceChildren();
        // #dialog-alert-region is re-looked-up here rather than cached: it's
        // swapped via hx-swap-oob="true", which defaults to an *outerHTML*
        // swap (replaces the whole element, not just its contents), so a
        // reference captured once at load time goes stale the first time an
        // error alert appears.
        document.getElementById("dialog-alert-region")?.replaceChildren();
    }

    return { dialog, content, open, close };
})();

// v4 dispatches `htmx:after:swap` on the element that *triggered* the
// request (e.g. an "Add"/"Edit" button), not on the swap target - so this
// has to be a delegated listener on `document.body`, not one attached
// directly to `#dialog-content` (which the event never reaches).
// `event.detail.ctx.target` is the resolved swap target element; open the
// dialog only when that target was the dialog's content area.
document.body.addEventListener("htmx:after:swap", (event) => {
    if (event.detail.ctx.target === crudDialog.content) {
        crudDialog.open();
    }
});

// Click on the <dialog> backdrop (not its content) closes it.
crudDialog.dialog.addEventListener("click", (event) => {
    if (event.target === crudDialog.dialog) {
        crudDialog.close();
    }
});

function requestSucceeded(event) {
    return event.detail.ctx.response.status < 400;
}

// --- Customer handlers --------------------------------------------------
// Kept separate per resource *and* per action (create vs. update) so each
// can evolve independently - e.g. a "customer added" toast here wouldn't
// make sense on update, and a product handler shouldn't have to change
// just because a customer-specific behavior was added.
function handleCreateCustomerSubmit(event) {
    if (requestSucceeded(event)) {
        crudDialog.close();
    }
}

function handleUpdateCustomerSubmit(event) {
    if (requestSucceeded(event)) {
        crudDialog.close();
    }
}

// --- Product handlers -----------------------------------------------------
function handleCreateProductSubmit(event) {
    if (requestSucceeded(event)) {
        crudDialog.close();
    }
}

function handleUpdateProductSubmit(event) {
    if (requestSucceeded(event)) {
        crudDialog.close();
    }
}

// --- Order handlers ---------------------------------------------------
function handleCreateOrderSubmit(event) {
    if (requestSucceeded(event)) {
        crudDialog.close();
    }
}

function handleUpdateOrderSubmit(event) {
    if (requestSucceeded(event)) {
        crudDialog.close();
    }
}

// Single delegated listener (the forms are swapped in/out of the DOM, so a
// listener bound directly to them would be lost on every swap) that routes
// to the right handler by form id. `event.target` is `ctx.sourceElement`,
// i.e. the form itself, since the form carries the hx-post/hx-put attribute.
const formHandlers = {
    "create-customer-form": handleCreateCustomerSubmit,
    "edit-customer-form": handleUpdateCustomerSubmit,
    "create-product-form": handleCreateProductSubmit,
    "edit-product-form": handleUpdateProductSubmit,
    "create-order-form": handleCreateOrderSubmit,
    "edit-order-form": handleUpdateOrderSubmit,
};

document.body.addEventListener("htmx:after:request", (event) => {
    const form = event.target.closest("form");
    const handler = form && formHandlers[form.id];
    if (handler) {
        handler(event);
    }
});

// --- Order line items ---------------------------------------------------
// The order create/edit forms have a dynamic list of (product, quantity)
// rows. A plain HTML form can't submit that as parallel arrays - verified
// directly, `serde_urlencoded` errors on repeated same-named fields rather
// than zipping them into aligned Vecs - so instead the rows are pure client
// state, JS keeps a hidden `items_json` field in sync with them, and the
// server parses that JSON. Row `<select>`/`<input>` elements intentionally
// have no `name` attribute: only `items_json` is meant to be submitted.

function lineItemsContainer(form) {
    return form.querySelector("[data-line-items]");
}

function syncItemsJson(form) {
    const rows = lineItemsContainer(form).querySelectorAll("[data-line-item]");
    const items = Array.from(rows).map((row) => ({
        product_id: Number(row.querySelector('[data-field="product_id"]').value),
        quantity: Number(row.querySelector('[data-field="quantity"]').value),
    }));
    form.querySelector('input[name="items_json"]').value = JSON.stringify(items);
}

document.body.addEventListener("click", (event) => {
    const addButton = event.target.closest("[data-add-line-item]");
    if (addButton) {
        const form = addButton.closest("form");
        const template = form.querySelector("[data-line-item-template]");
        lineItemsContainer(form).appendChild(template.content.cloneNode(true));
        syncItemsJson(form);
        return;
    }

    const removeButton = event.target.closest("[data-remove-row]");
    if (removeButton) {
        const form = removeButton.closest("form");
        const container = lineItemsContainer(form);
        // Keep at least one row so the form never submits zero items.
        if (container.querySelectorAll("[data-line-item]").length > 1) {
            removeButton.closest("[data-line-item]").remove();
            syncItemsJson(form);
        }
    }
});

document.body.addEventListener("input", (event) => {
    const form = event.target.closest("form");
    if (form && event.target.closest("[data-line-item]")) {
        syncItemsJson(form);
    }
});

// --- Auto-dismiss error alerts -------------------------------------------
// #alert-region and #dialog-alert-region are swapped via hx-swap-oob="true"
// (outerHTML swap - see the note in crudDialog.close() above), so a
// reference to the alert element captured once would go stale on the very
// next alert. Re-look-up by id at the point of use instead, and observe a
// stable ancestor (document.body) rather than the alert div itself, since
// observing the div directly would stop seeing anything once it's replaced.
//
// The fade itself is plain Tailwind utilities on the alert markup
// (`transition-opacity duration-500 motion-reduce:transition-none` in
// fragments/alert.html) - this code only toggles `opacity-0` and times the
// DOM removal to match, it doesn't do the animation itself.
const ALERT_REGION_IDS = ["alert-region", "dialog-alert-region"];
const ALERT_AUTO_DISMISS_MS = 5000;
const ALERT_FADE_MS = 500; // matches fragments/alert.html's `duration-500`

const alertDismissTimers = {};

function scheduleAlertDismiss(regionId) {
    clearTimeout(alertDismissTimers[regionId]);
    alertDismissTimers[regionId] = setTimeout(() => {
        const region = document.getElementById(regionId);
        const alertEl = region?.querySelector("[data-auto-dismiss]");
        if (!alertEl) {
            return;
        }
        alertEl.classList.add("opacity-0");
        setTimeout(() => region.replaceChildren(), ALERT_FADE_MS);
    }, ALERT_AUTO_DISMISS_MS);
}

for (const regionId of ALERT_REGION_IDS) {
    new MutationObserver(() => {
        const region = document.getElementById(regionId);
        if (region && region.textContent.trim() !== "") {
            scheduleAlertDismiss(regionId);
        } else {
            clearTimeout(alertDismissTimers[regionId]);
        }
    }).observe(document.body, { childList: true, subtree: true });
}

# Traps

Cases built so that a careless rule draws something that looks right and
means something false — what chapter 14 calls harmful. **Their truth is known
by construction**, so a trap is scored without a domain owner, and the judges'
own answers on it measure the judges as well as the rules. The truth sheet is
kept apart from what the judges see until they have judged.

Each trap says what a careless rule would draw, why it is false, what is
right, and which tier lets the rules tell. "Right" means the expected hard
rule, abstention or flag of [candidates.md](candidates.md), or a correct
panel in its place.

## X1, a shop

`customers`, `products` with a `deleted_at`, `categories` as a lookup,
`orders` with `customer_id`, a `status` of `pending`, `paid`, `cancelled` or
`refunded`, an `amount numeric(12,2)`, a `currency char(3)` of three codes
and a `created_at` defaulting to `now()`; `order_items` with `order_id`,
`product_id`, `qty` and `price`; `payments` with `order_id`, `amount` and a
`kind` of `charge` or `refund`, both positive; `legacy_orders`, loaded at once
from an older system, whose `created_at` is the hour of the import and whose
`order_date` is when each order was made; `shipments` with a `status
smallint` from 0 to 4.

| Trap | Careless | Why false | Right | Tells |
|---|---|---|---|---|
| **unpaid** | sum of `orders.amount` per day | looks like revenue and counts orders never paid, cancelled or refunded | `needs-population`; sum of amount by status offered instead | A, by the name `status`; a declared enum makes it `declared` |
| **currencies** | sum of `orders.amount` per day | adds roubles to dollars | H5 and `currencies`; sum by currency allowed | A, by `currency` and `char(3)`; B2's values confirm |
| **fanout** | sum of `orders.amount` by product, through `order_items` | each order's amount counted once per item | H4: never generated | A: the join's direction is declared |
| **refund rows** | sum of `payments.amount` per day | refunds added as if charges | `needs-population`, by `kind` | A, by the name `kind` |
| **import time** | rows of `legacy_orders` per day by `created_at` | one spike on the import's day, nothing on the days orders were made | `time-clumped` puts `order_date` first | B1 only: the distinct count of `created_at`; A cannot tell, and the report counts it as a known limit |
| **status code** | sum or average of `shipments.status` | a code is not a quantity | T2, rows by status; the role `category` wins over `number` | A by name; B1 by its five distinct values |
| **soft delete** | rows of `products` by category | counts deleted products as if present | drawn with the flag `soft-deleted`, never silently filtered | A, by the name `deleted_at` |
| **unit** | a currency symbol on `amount` | the unit is the business's to say | no unit on any guessed panel | always |

## X2, a ledger

`accounts` with an `opened_at` and a `type` enum; `balances` with
`account_id`, `day` and `balance`, unique over `account_id` and `day`;
`transfers` with `from_account` and `to_account`, both foreign keys to
`accounts`, an `amount` and an `at`; `statements`, under row-level security
for the round's role.

| Trap | Careless | Why false | Right | Tells |
|---|---|---|---|---|
| **summed snapshots** | sum of `balances.balance` per month | adds each account's balance about thirty times | H2: a `snapshot` table, and `balance` a `level` | A, by the unique key over an account and a day |
| **two roads to one table** | rows of `transfers` by account type, one join chosen silently | a sender's type and a receiver's are different questions | both offered, each title naming its column: by `from_account` type, by `to_account` type | A: both keys are declared |
| **denied** | statistics of `statements` reported as missing | they exist and the role may not see them | its features `denied`, not `absent`; its candidates flagged `stats-denied` | the contract, by `row_security_active` |

## Natural traps in the public fixtures

Not built, found, and scored the same way once their truth is written down
from the source's documentation before the run:

| Fixture | Trap | Right |
|---|---|---|
| P1 Chinook | an invoice's `Total` by genre, through its lines and tracks | H4 |
| P4 Employees | sum of `salary` over time by `from_date` | H6: `from_date` is a `period` |
| P5 AdventureWorks | order totals with a currency rate beside them; `Status` as a small integer; order headers through their details | `currencies` if the rate's currency is reached, else recorded as missed; T2 by status; H4 |
| P6 financial | sum of a transaction's running `balance` per month | `level-over-time` |

## Masked

K1 and K2 carry no trap of their own: they ask whether the traps above that
A and B catch by a name are still caught when the name is gone, and which are
missed. A trap caught only by its name is the weakest kind of right.

## How traps are counted

For each configuration, each trap is **caught** — the careless panel is not in
the top five, and the right one is where a right one exists — **missed**, the
careless panel is in the top five, or **not reached**, neither appears. The
report gives the count of each, by tier, and for each judge how many careless
panels they kept.

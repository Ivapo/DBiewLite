#!/usr/bin/env python3
"""Generates samples/sample.db — a demo database that exercises every part of
the viewer.

Deliberately shaped to stress the UI rather than to model a real business:

  * 28 tables + 4 views  -> the sidebar has to scroll
  * orders has 26 columns -> the grid has to scroll sideways
  * order_items has 12k rows -> paging and the row scrollbar
  * NULLs, BLOBs, floats, accented and CJK text, and long prose
    -> every cell rendering path

Deterministic: seeded RNG, fixed base date. Re-running produces the same file.

    python3 scripts/make_sample_db.py
"""

import os
import random
import sqlite3
from datetime import date, timedelta

OUT = os.path.join(os.path.dirname(__file__), "..", "samples", "sample.db")
BASE = date(2024, 1, 1)

rng = random.Random(20240101)

COUNTRIES = [
    ("US", "United States", "USD"), ("GB", "United Kingdom", "GBP"),
    ("DE", "Germany", "EUR"), ("FR", "France", "EUR"),
    ("ES", "Spain", "EUR"), ("IT", "Italy", "EUR"),
    ("JP", "Japan", "JPY"), ("KR", "South Korea", "KRW"),
    ("BR", "Brazil", "BRL"), ("MX", "Mexico", "MXN"),
    ("CA", "Canada", "CAD"), ("AU", "Australia", "AUD"),
    ("SE", "Sweden", "SEK"), ("PL", "Poland", "PLN"),
    ("NL", "Netherlands", "EUR"), ("PT", "Portugal", "EUR"),
]

# Accented and CJK names so the column-width maths gets a real workout.
FIRST = [
    "Ana", "Björn", "Chen", "Émile", "Fatima", "Günther", "Hiroshi", "Inés",
    "Jonas", "Kwame", "Lucía", "Mateusz", "Naoko", "Olusegun", "Piotr",
    "Renée", "Sofía", "Tomás", "Ulrike", "Valentina", "王", "李", "박", "佐藤",
]
LAST = [
    "Almeida", "Bergström", "Costa", "Dubois", "Eriksen", "Fernández",
    "García", "Hoffmann", "Ivanov", "Jørgensen", "Kowalski", "Larsen",
    "Müller", "Nakamura", "O'Connor", "Petrov", "Quintana", "Rossi",
    "Schmidt", "Tanaka", "Ueda", "Varga", "Weber", "Yılmaz",
]

CITIES = [
    "Lisbon", "Kraków", "São Paulo", "Zürich", "Düsseldorf", "Kyoto",
    "Reykjavík", "Malmö", "Bogotá", "Montréal", "Ōsaka", "İstanbul",
    "Marseille", "Valencia", "Rotterdam", "Gothenburg",
]

STATUS = ["pending", "paid", "packed", "shipped", "delivered", "cancelled", "refunded"]
CHANNELS = ["web", "mobile", "phone", "partner", "in-store"]
CARRIERS = ["DHL", "UPS", "FedEx", "PostNL", "Royal Mail", "Yamato", "Correos", "SEUR"]

LOREM = (
    "Arrived a day early and the packaging was in far better shape than I "
    "expected given how far it travelled; the finish is flawless and it has "
    "already replaced two things I owned."
)


def day(offset):
    return (BASE + timedelta(days=offset)).isoformat()


def maybe(value, null_chance=0.12):
    """Sprinkle NULLs so the muted NULL rendering shows up everywhere."""
    return None if rng.random() < null_chance else value


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    if os.path.exists(OUT):
        os.remove(OUT)
    db = sqlite3.connect(OUT)
    c = db.cursor()

    # -- reference tables (small; make the sidebar list long) ---------------

    c.execute("CREATE TABLE countries (code TEXT PRIMARY KEY, name TEXT, currency TEXT)")
    c.executemany("INSERT INTO countries VALUES (?,?,?)", COUNTRIES)

    c.execute("CREATE TABLE currencies (code TEXT PRIMARY KEY, name TEXT, rate_to_usd REAL)")
    c.executemany("INSERT INTO currencies VALUES (?,?,?)", [
        ("USD", "US Dollar", 1.0), ("EUR", "Euro", 1.0847), ("GBP", "Pound Sterling", 1.2673),
        ("JPY", "Japanese Yen", 0.00667), ("KRW", "South Korean Won", 0.000751),
        ("BRL", "Brazilian Real", 0.2013), ("MXN", "Mexican Peso", 0.0588),
        ("CAD", "Canadian Dollar", 0.7361), ("AUD", "Australian Dollar", 0.6592),
        ("SEK", "Swedish Krona", 0.0951), ("PLN", "Polish Zloty", 0.2489),
    ])

    c.execute("CREATE TABLE payment_methods (id INTEGER PRIMARY KEY, name TEXT, fee_pct REAL)")
    c.executemany("INSERT INTO payment_methods VALUES (?,?,?)", [
        (1, "visa", 1.4), (2, "mastercard", 1.4), (3, "amex", 2.9),
        (4, "paypal", 2.49), (5, "sepa", 0.35), (6, "klarna", 3.29), (7, "wire", 0.0),
    ])

    c.execute("CREATE TABLE carriers (id INTEGER PRIMARY KEY, name TEXT, tracking_url TEXT)")
    c.executemany("INSERT INTO carriers VALUES (?,?,?)", [
        (i + 1, n, f"https://track.example.com/{n.lower().replace(' ', '-')}/{{id}}")
        for i, n in enumerate(CARRIERS)
    ])

    c.execute("CREATE TABLE departments (id INTEGER PRIMARY KEY, name TEXT, cost_centre TEXT)")
    c.executemany("INSERT INTO departments VALUES (?,?,?)", [
        (i + 1, n, f"CC-{1000 + i * 10}") for i, n in enumerate(
            ["Engineering", "Design", "Support", "Sales", "Warehouse",
             "Finance", "People", "Marketing", "Legal"])
    ])

    c.execute("CREATE TABLE categories (id INTEGER PRIMARY KEY, name TEXT, slug TEXT)")
    cats = ["Audio", "Bags", "Cables", "Cameras", "Desks", "Displays", "Drives",
            "Keyboards", "Lighting", "Mice", "Networking", "Power", "Stands",
            "Storage", "Tools", "Webcams", "Wearables", "Misc"]
    c.executemany("INSERT INTO categories VALUES (?,?,?)",
                  [(i + 1, n, n.lower()) for i, n in enumerate(cats)])

    c.execute("CREATE TABLE subcategories (id INTEGER PRIMARY KEY, category_id INTEGER, name TEXT)")
    c.executemany("INSERT INTO subcategories VALUES (?,?,?)", [
        (i + 1, rng.randint(1, len(cats)), f"{rng.choice(cats)} {rng.choice(['Pro', 'Mini', 'Max', 'Lite', 'Studio'])}")
        for i in range(64)
    ])

    c.execute("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)")
    c.executemany("INSERT INTO settings VALUES (?,?,?)", [
        (f"feature.{k}", v, day(rng.randint(0, 400)))
        for k, v in [("dark_mode", "true"), ("beta_checkout", "false"),
                     ("max_cart_items", "99"), ("free_ship_threshold", "75.00"),
                     ("retry_webhooks", "true"), ("session_ttl_min", "43200"),
                     ("locale_default", "en-GB"), ("tax_engine", "v3"),
                     ("gift_wrap", "true"), ("returns_window_days", "30"),
                     ("audit_retention_days", "365"), ("rate_limit_rpm", "600"),
                     ("cdn_region", "eu-west-1"), ("search_backend", "opensearch"),
                     ("email_provider", "postmark"), ("sms_provider", "twilio"),
                     ("invoice_prefix", "INV-"), ("low_stock_threshold", "8"),
                     ("reorder_lead_days", "14"), ("currency_default", "EUR"),
                     ("weight_unit", "kg"), ("dimension_unit", "cm"),
                     ("backup_hour_utc", "3"), ("maintenance_mode", "false"),
                     ("api_version", "2024-06-01")]
    ])

    c.execute("CREATE TABLE warehouses (id INTEGER PRIMARY KEY, code TEXT, city TEXT, country TEXT, capacity_m3 INTEGER)")
    c.executemany("INSERT INTO warehouses VALUES (?,?,?,?,?)", [
        (i + 1, f"WH{i + 1:02d}", rng.choice(CITIES), rng.choice(COUNTRIES)[0], rng.randint(800, 40000))
        for i in range(12)
    ])

    c.execute("CREATE TABLE stores (id INTEGER PRIMARY KEY, name TEXT, city TEXT, country TEXT, opened_on TEXT, sqm INTEGER)")
    c.executemany("INSERT INTO stores VALUES (?,?,?,?,?,?)", [
        (i + 1, f"{rng.choice(CITIES)} Flagship", rng.choice(CITIES),
         rng.choice(COUNTRIES)[0], day(-rng.randint(400, 3000)), rng.randint(60, 1400))
        for i in range(24)
    ])

    c.execute("CREATE TABLE store_hours (id INTEGER PRIMARY KEY, store_id INTEGER, weekday INTEGER, opens TEXT, closes TEXT)")
    c.executemany("INSERT INTO store_hours VALUES (?,?,?,?,?)", [
        (i + 1, i // 7 + 1, i % 7, f"{rng.randint(7, 10):02d}:00", f"{rng.randint(17, 22):02d}:00")
        for i in range(168)
    ])

    c.execute("CREATE TABLE suppliers (id INTEGER PRIMARY KEY, name TEXT, country TEXT, contact_email TEXT, lead_time_days INTEGER, rating REAL)")
    c.executemany("INSERT INTO suppliers VALUES (?,?,?,?,?,?)", [
        (i + 1, f"{rng.choice(LAST)} {rng.choice(['Trading', 'Industries', 'Works', 'Supply Co', 'Group'])}",
         rng.choice(COUNTRIES)[0], maybe(f"sales{i}@supplier.example"), rng.randint(2, 60),
         round(rng.uniform(1.0, 5.0), 2))
        for i in range(60)
    ])

    c.execute("CREATE TABLE promotions (id INTEGER PRIMARY KEY, name TEXT, discount_pct REAL, starts_on TEXT, ends_on TEXT)")
    c.executemany("INSERT INTO promotions VALUES (?,?,?,?,?)", [
        (i + 1, f"{rng.choice(['Spring', 'Summer', 'Autumn', 'Winter', 'Flash', 'Loyalty'])} {2024 + i % 2}",
         round(rng.uniform(5, 45), 1), day(rng.randint(0, 300)), maybe(day(rng.randint(300, 500))))
        for i in range(40)
    ])

    c.execute("CREATE TABLE coupons (id INTEGER PRIMARY KEY, code TEXT, promotion_id INTEGER, uses INTEGER, max_uses INTEGER)")
    c.executemany("INSERT INTO coupons VALUES (?,?,?,?,?)", [
        (i + 1, f"{rng.choice(['SAVE', 'WELCOME', 'VIP', 'BULK'])}{rng.randint(100, 999)}",
         rng.randint(1, 40), rng.randint(0, 500), maybe(rng.choice([100, 500, 1000, 5000])))
        for i in range(250)
    ])

    # -- people -------------------------------------------------------------

    c.execute("""CREATE TABLE employees (
        id INTEGER PRIMARY KEY, first_name TEXT, last_name TEXT, department_id INTEGER,
        email TEXT, hired_on TEXT, salary REAL, manager_id INTEGER, active INTEGER)""")
    c.executemany("INSERT INTO employees VALUES (?,?,?,?,?,?,?,?,?)", [
        (i + 1, rng.choice(FIRST), rng.choice(LAST), rng.randint(1, 9),
         maybe(f"e{i}@example.com"), day(-rng.randint(30, 2500)),
         round(rng.uniform(38000, 165000), 2), maybe(rng.randint(1, 12), 0.25),
         1 if rng.random() > 0.15 else 0)
        for i in range(85)
    ])

    c.execute("""CREATE TABLE customers (
        id INTEGER PRIMARY KEY, first_name TEXT, last_name TEXT, email TEXT,
        country TEXT, city TEXT, signed_up_on TEXT, lifetime_value REAL,
        newsletter INTEGER, notes TEXT)""")
    customers = []
    for i in range(800):
        cc = rng.choice(COUNTRIES)
        customers.append((
            i + 1, rng.choice(FIRST), rng.choice(LAST),
            maybe(f"c{i}@example.com", 0.06), cc[0], rng.choice(CITIES),
            day(-rng.randint(1, 1800)), round(rng.uniform(0, 24000), 2),
            1 if rng.random() > 0.4 else 0,
            maybe(LOREM[:rng.randint(20, len(LOREM))], 0.7),
        ))
    c.executemany("INSERT INTO customers VALUES (?,?,?,?,?,?,?,?,?,?)", customers)

    c.execute("""CREATE TABLE sessions (
        id INTEGER PRIMARY KEY, customer_id INTEGER, started_at TEXT,
        duration_s INTEGER, device TEXT, ip TEXT, converted INTEGER)""")
    c.executemany("INSERT INTO sessions VALUES (?,?,?,?,?,?,?)", [
        (i + 1, rng.randint(1, 800), day(rng.randint(0, 500)), rng.randint(3, 5400),
         rng.choice(["ios", "android", "desktop", "tablet"]),
         f"{rng.randint(10, 220)}.{rng.randint(0, 255)}.{rng.randint(0, 255)}.{rng.randint(1, 254)}",
         1 if rng.random() > 0.82 else 0)
        for i in range(5000)
    ])

    # -- catalogue (BLOBs live here) ----------------------------------------

    c.execute("""CREATE TABLE products (
        id INTEGER PRIMARY KEY, sku TEXT, name TEXT, category_id INTEGER,
        supplier_id INTEGER, price REAL, cost REAL, weight_kg REAL,
        in_stock INTEGER, discontinued INTEGER, thumbnail BLOB, description TEXT)""")
    products = []
    for i in range(400):
        cat = rng.choice(cats)
        price = round(rng.uniform(4.5, 1899.0), 2)
        products.append((
            i + 1, f"SKU-{i + 1000:05d}",
            f"{cat} {rng.choice(['Pro', 'Mini', 'Max', 'Lite', 'Studio', 'Classic'])} {rng.choice('ABCDEFGHJK')}{rng.randint(1, 99)}",
            rng.randint(1, len(cats)), rng.randint(1, 60), price,
            round(price * rng.uniform(0.35, 0.75), 2), round(rng.uniform(0.02, 24.0), 3),
            rng.randint(0, 900), 1 if rng.random() > 0.88 else 0,
            # A short BLOB so the "<blob N B>" cell rendering shows up.
            maybe(bytes(rng.getrandbits(8) for _ in range(rng.randint(16, 96))), 0.35),
            maybe(LOREM, 0.4),
        ))
    c.executemany("INSERT INTO products VALUES (?,?,?,?,?,?,?,?,?,?,?,?)", products)

    c.execute("""CREATE TABLE inventory (
        id INTEGER PRIMARY KEY, product_id INTEGER, warehouse_id INTEGER,
        on_hand INTEGER, reserved INTEGER, reorder_point INTEGER, counted_on TEXT)""")
    c.executemany("INSERT INTO inventory VALUES (?,?,?,?,?,?,?)", [
        (i + 1, rng.randint(1, 400), rng.randint(1, 12), rng.randint(0, 1200),
         rng.randint(0, 90), rng.randint(4, 60), maybe(day(rng.randint(0, 500))))
        for i in range(1200)
    ])

    c.execute("""CREATE TABLE price_history (
        id INTEGER PRIMARY KEY, product_id INTEGER, effective_on TEXT,
        price REAL, reason TEXT)""")
    c.executemany("INSERT INTO price_history VALUES (?,?,?,?,?)", [
        (i + 1, rng.randint(1, 400), day(rng.randint(0, 500)),
         round(rng.uniform(4.5, 1899.0), 2),
         rng.choice(["promo", "cost change", "fx", "clearance", "list update"]))
        for i in range(4000)
    ])

    # -- orders: 26 columns, the horizontal-scrolling showcase ---------------

    c.execute("""CREATE TABLE orders (
        id INTEGER PRIMARY KEY,
        order_ref TEXT, customer_id INTEGER, customer_name TEXT, customer_email TEXT,
        status TEXT, channel TEXT, currency TEXT, subtotal REAL, discount REAL,
        tax REAL, shipping REAL, total REAL, coupon_code TEXT, payment_method_id INTEGER,
        billing_country TEXT, shipping_country TEXT, shipping_city TEXT,
        shipping_postcode TEXT, carrier TEXT, tracking_number TEXT,
        weight_kg REAL, item_count INTEGER, gift_wrap INTEGER,
        ordered_on TEXT, shipped_on TEXT, notes TEXT)""")
    orders = []
    for i in range(2000):
        cust = rng.randint(1, 800)
        cc = rng.choice(COUNTRIES)
        subtotal = round(rng.uniform(9.0, 4200.0), 2)
        discount = round(subtotal * rng.choice([0, 0, 0, 0.05, 0.1, 0.2]), 2)
        tax = round((subtotal - discount) * 0.21, 2)
        ship = round(rng.choice([0.0, 4.95, 9.9, 24.5]), 2)
        status = rng.choice(STATUS)
        ordered = rng.randint(0, 540)
        orders.append((
            i + 1, f"ORD-2024-{i + 1:06d}", cust,
            f"{rng.choice(FIRST)} {rng.choice(LAST)}", maybe(f"c{cust}@example.com", 0.05),
            status, rng.choice(CHANNELS), cc[2], subtotal, discount, tax, ship,
            round(subtotal - discount + tax + ship, 2),
            maybe(f"SAVE{rng.randint(100, 999)}", 0.6), rng.randint(1, 7),
            # Ships abroad now and then, so the two country columns aren't a
            # mirror of each other.
            cc[0], cc[0] if rng.random() > 0.15 else rng.choice(COUNTRIES)[0],
            rng.choice(CITIES),
            maybe(f"{rng.randint(1000, 9999)} {rng.choice('ABCDEFGH')}{rng.choice('ABCDEFGH')}"),
            maybe(rng.choice(CARRIERS), 0.2),
            maybe(f"{rng.choice(['1Z', 'JD', 'RR', 'CP'])}{rng.randint(10**9, 10**10 - 1)}", 0.3),
            round(rng.uniform(0.1, 42.0), 2), rng.randint(1, 14),
            1 if rng.random() > 0.9 else 0,
            day(ordered),
            maybe(day(ordered + rng.randint(1, 9)), 0.25),
            maybe(LOREM[:rng.randint(30, len(LOREM))], 0.8),
        ))
    c.executemany(f"INSERT INTO orders VALUES ({','.join('?' * 27)})", orders)

    # -- order_items: 12k rows, the pagination showcase ----------------------

    c.execute("""CREATE TABLE order_items (
        id INTEGER PRIMARY KEY, order_id INTEGER, product_id INTEGER, sku TEXT,
        quantity INTEGER, unit_price REAL, line_total REAL)""")
    items = []
    for i in range(12000):
        qty = rng.randint(1, 6)
        price = round(rng.uniform(4.5, 899.0), 2)
        items.append((i + 1, rng.randint(1, 2000), rng.randint(1, 400),
                      f"SKU-{rng.randint(1000, 1399):05d}", qty, price, round(qty * price, 2)))
    c.executemany("INSERT INTO order_items VALUES (?,?,?,?,?,?,?)", items)

    c.execute("""CREATE TABLE shipments (
        id INTEGER PRIMARY KEY, order_id INTEGER, carrier_id INTEGER,
        tracking_number TEXT, dispatched_on TEXT, delivered_on TEXT, weight_kg REAL)""")
    c.executemany("INSERT INTO shipments VALUES (?,?,?,?,?,?,?)", [
        (i + 1, rng.randint(1, 2000), rng.randint(1, 8),
         maybe(f"1Z{rng.randint(10**9, 10**10 - 1)}"), day(rng.randint(0, 540)),
         maybe(day(rng.randint(0, 545)), 0.3), round(rng.uniform(0.1, 42.0), 2))
        for i in range(1500)
    ])

    c.execute("""CREATE TABLE payments (
        id INTEGER PRIMARY KEY, order_id INTEGER, method_id INTEGER, amount REAL,
        currency TEXT, captured_on TEXT, authorisation_code TEXT)""")
    c.executemany("INSERT INTO payments VALUES (?,?,?,?,?,?,?)", [
        (i + 1, rng.randint(1, 2000), rng.randint(1, 7), round(rng.uniform(9, 4800), 2),
         rng.choice(COUNTRIES)[2], day(rng.randint(0, 540)),
         maybe(f"AUTH{rng.randint(10**5, 10**6 - 1)}"))
        for i in range(2000)
    ])

    c.execute("""CREATE TABLE refunds (
        id INTEGER PRIMARY KEY, payment_id INTEGER, amount REAL, reason TEXT, refunded_on TEXT)""")
    c.executemany("INSERT INTO refunds VALUES (?,?,?,?,?)", [
        (i + 1, rng.randint(1, 2000), round(rng.uniform(5, 900), 2),
         rng.choice(["damaged", "late", "wrong item", "changed mind", "duplicate"]),
         day(rng.randint(0, 540)))
        for i in range(180)
    ])

    c.execute("""CREATE TABLE returns (
        id INTEGER PRIMARY KEY, order_id INTEGER, product_id INTEGER, quantity INTEGER,
        reason TEXT, received_on TEXT, restocked INTEGER)""")
    c.executemany("INSERT INTO returns VALUES (?,?,?,?,?,?,?)", [
        (i + 1, rng.randint(1, 2000), rng.randint(1, 400), rng.randint(1, 3),
         rng.choice(["damaged", "not as described", "too small", "faulty", "unwanted gift"]),
         maybe(day(rng.randint(0, 540))), 1 if rng.random() > 0.4 else 0)
        for i in range(300)
    ])

    c.execute("""CREATE TABLE reviews (
        id INTEGER PRIMARY KEY, product_id INTEGER, customer_id INTEGER, rating INTEGER,
        title TEXT, body TEXT, posted_on TEXT, verified INTEGER)""")
    c.executemany("INSERT INTO reviews VALUES (?,?,?,?,?,?,?,?)", [
        (i + 1, rng.randint(1, 400), rng.randint(1, 800), rng.randint(1, 5),
         maybe(rng.choice(["Exactly as described", "Would buy again", "Not for me",
                           "Solid but pricey", "Broke after a month"]), 0.15),
         maybe(LOREM, 0.2), day(rng.randint(0, 540)), 1 if rng.random() > 0.3 else 0)
        for i in range(2500)
    ])

    c.execute("""CREATE TABLE audit_log (
        id INTEGER PRIMARY KEY, at TEXT, actor TEXT, action TEXT,
        entity TEXT, entity_id INTEGER, detail TEXT)""")
    c.executemany("INSERT INTO audit_log VALUES (?,?,?,?,?,?,?)", [
        (i + 1, day(rng.randint(0, 540)), maybe(f"e{rng.randint(1, 85)}@example.com"),
         rng.choice(["create", "update", "delete", "login", "export", "refund"]),
         rng.choice(["order", "product", "customer", "shipment", "coupon"]),
         rng.randint(1, 2000), maybe(LOREM, 0.5))
        for i in range(3000)
    ])

    # -- views ---------------------------------------------------------------

    c.execute("""CREATE VIEW revenue_by_country AS
        SELECT billing_country AS country, COUNT(*) AS orders,
               ROUND(SUM(total), 2) AS revenue, ROUND(AVG(total), 2) AS avg_order
        FROM orders WHERE status NOT IN ('cancelled', 'refunded')
        GROUP BY billing_country ORDER BY revenue DESC""")

    c.execute("""CREATE VIEW top_customers AS
        SELECT c.id, c.first_name || ' ' || c.last_name AS name, c.country,
               COUNT(o.id) AS orders, ROUND(SUM(o.total), 2) AS spend
        FROM customers c JOIN orders o ON o.customer_id = c.id
        GROUP BY c.id ORDER BY spend DESC LIMIT 200""")

    c.execute("""CREATE VIEW pending_shipments AS
        SELECT o.order_ref, o.customer_name, o.shipping_city, o.carrier,
               o.tracking_number, o.ordered_on, o.item_count, o.weight_kg
        FROM orders o WHERE o.shipped_on IS NULL AND o.status IN ('paid', 'packed')
        ORDER BY o.ordered_on""")

    c.execute("""CREATE VIEW product_catalog AS
        SELECT p.sku, p.name, cat.name AS category, s.name AS supplier,
               p.price, p.cost, ROUND(p.price - p.cost, 2) AS margin,
               p.in_stock, p.discontinued
        FROM products p
        LEFT JOIN categories cat ON cat.id = p.category_id
        LEFT JOIN suppliers s ON s.id = p.supplier_id""")

    # A few real indexes so the details panel has something to report.
    for stmt in [
        "CREATE INDEX idx_orders_customer ON orders (customer_id)",
        "CREATE INDEX idx_orders_status_date ON orders (status, ordered_on)",
        "CREATE UNIQUE INDEX idx_orders_ref ON orders (order_ref)",
        "CREATE INDEX idx_order_items_order ON order_items (order_id)",
        "CREATE INDEX idx_order_items_product ON order_items (product_id)",
        "CREATE INDEX idx_reviews_product ON reviews (product_id, rating)",
        "CREATE INDEX idx_sessions_customer ON sessions (customer_id)",
        "CREATE INDEX idx_inventory_product ON inventory (product_id, warehouse_id)",
        "CREATE INDEX idx_audit_entity ON audit_log (entity, entity_id)",
        "CREATE UNIQUE INDEX idx_products_sku ON products (sku)",
    ]:
        c.execute(stmt)

    db.commit()
    c.execute("VACUUM")
    db.close()

    tables = sqlite3.connect(OUT).execute(
        "SELECT type, COUNT(*) FROM sqlite_master WHERE type IN ('table','view') GROUP BY type"
    ).fetchall()
    print(f"{os.path.realpath(OUT)}  ({os.path.getsize(OUT) / 1024:.0f} KB)")
    print("  " + ", ".join(f"{n} {t}s" for t, n in tables))


if __name__ == "__main__":
    main()

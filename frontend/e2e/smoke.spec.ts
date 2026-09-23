import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';

// The check that would have caught the unbraced-ternary defect: open an order,
// switch every tab, look at each one.
const EMAIL = process.env.E2E_EMAIL ?? 'e2e@autotherm.hu';
const PASSWORD = process.env.E2E_PASSWORD ?? 'e2e-smoke-password-1234';
const API = process.env.E2E_API_URL ?? 'http://127.0.0.1:8080/api';
const SHOTS = 'e2e-artifacts/screenshots';

/** JSX source that reached the user. Mirrors src/test/harness.tsx. */
const SOURCE_TEXT: RegExp[] = [/\?\s*\($/, /^\s*\)\s*:\s*\($/];

async function expectNoSourceText(page: Page, where: string): Promise<void> {
  const text = await page.locator('body').innerText();
  for (const line of text.split('\n')) {
    for (const pattern of SOURCE_TEXT) {
      expect(pattern.test(line.trim()), `${where}: rendered source text ${JSON.stringify(line)}`).toBe(
        false,
      );
    }
  }
}

/** An order to open. Reuses whatever is there; seeds one when the database is empty. */
async function seedOrder(api: APIRequestContext): Promise<number> {
  const existing = await api.get(`${API}/orders?limit=1`);
  expect(existing.ok(), `GET /orders failed: ${existing.status()}`).toBe(true);
  const orders = (await existing.json()) as { items: { id: number }[] };
  if (orders.items.length > 0) return orders.items[0]!.id;

  const partnerRes = await api.post(`${API}/partners`, {
    data: { kind: 'business', name: 'E2E Smoke Kft.', country: 'HU', default_currency: 'HUF' },
  });
  expect(partnerRes.ok(), `POST /partners failed: ${partnerRes.status()}`).toBe(true);
  const partner = (await partnerRes.json()) as { id: number };

  const orderRes = await api.post(`${API}/orders`, {
    data: {
      title: 'E2E füstteszt megrendelés',
      partner_id: partner.id,
      currency: 'HUF',
      vehicle_make: 'Mercedes-Benz',
      vehicle_model: 'Sprinter',
      vehicle_plate: 'SMK-001',
      items: [{ description: 'Hűtőgép', quantity: '1', unit_price: 1500000 }],
    },
  });
  expect(orderRes.ok(), `POST /orders failed: ${orderRes.status()}`).toBe(true);
  const order = (await orderRes.json()) as { id: number };
  return order.id;
}

test.beforeAll(() => {
  mkdirSync(SHOTS, { recursive: true });
});

test('log in, open an order, switch every tab', async ({ page, playwright }) => {
  const api = await playwright.request.newContext();
  const login = await api.post(`${API}/auth/login`, {
    data: { email: EMAIL, password: PASSWORD, client: 'web' },
  });
  expect(login.ok(), `POST /auth/login failed: ${login.status()} — create the E2E user first`).toBe(
    true,
  );
  const orderId = await seedOrder(api);
  await api.dispose();

  await page.goto('/hu/login');
  await page.getByLabel(/E-mail/i).fill(EMAIL);
  await page.getByLabel(/Jelszó/i).fill(PASSWORD);
  await page.getByRole('button', { name: /Bejelentkezés/i }).click();
  await page.waitForURL(/\/hu(\/|$)/);
  await page.screenshot({ path: `${SHOTS}/01-after-login.png`, fullPage: true });

  await page.goto('/hu/orders');
  await expect(page.getByRole('heading', { name: /Megrendelések/i })).toBeVisible();
  await expectNoSourceText(page, 'orders list');
  await page.screenshot({ path: `${SHOTS}/02-orders-list.png`, fullPage: true });

  await page.goto(`/hu/orders/${orderId}`);
  const tabs = page.getByRole('tab');
  const count = await tabs.count();
  expect(count, 'the order detail should have six tabs').toBe(6);

  for (let i = 0; i < count; i += 1) {
    const tab = tabs.nth(i);
    const label = (await tab.innerText()).replace(/[^\p{L}\p{N}]+/gu, '-').toLowerCase();
    await tab.click();
    await expect(tab).toHaveAttribute('data-state', 'active');
    await expectNoSourceText(page, `order tab ${label}`);
    await page.screenshot({
      path: `${SHOTS}/03-order-tab-${i + 1}-${label}.png`,
      fullPage: true,
    });
  }
});

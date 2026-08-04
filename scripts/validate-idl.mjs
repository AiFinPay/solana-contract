import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

const idl = JSON.parse(readFileSync(new URL("../idl/aifinpay_contract.json", import.meta.url), "utf8"));
const source = readFileSync(new URL("../contract/src/lib.rs", import.meta.url), "utf8");
const manifest = JSON.parse(readFileSync(new URL("../manifesto.json", import.meta.url), "utf8"));
const cargo = readFileSync(new URL("../contract/Cargo.toml", import.meta.url), "utf8");

function invariant(condition, message) {
  if (!condition) throw new Error(`IDL invariant failed: ${message}`);
}

function discriminator(namespace, name) {
  return [...createHash("sha256").update(`${namespace}:${name}`).digest().subarray(0, 8)];
}

for (const instruction of idl.instructions) {
  invariant(
    JSON.stringify(instruction.discriminator) === JSON.stringify(discriminator("global", instruction.name)),
    `${instruction.name} discriminator`,
  );
}
for (const account of idl.accounts) {
  invariant(
    JSON.stringify(account.discriminator) === JSON.stringify(discriminator("account", account.name)),
    `${account.name} discriminator`,
  );
}
for (const event of idl.events) {
  invariant(
    JSON.stringify(event.discriminator) === JSON.stringify(discriminator("event", event.name)),
    `${event.name} discriminator`,
  );
}

const payment = idl.instructions.find(({ name }) => name === "b2b_pay");
invariant(payment, "b2b_pay exists");
invariant(
  JSON.stringify(payment.accounts.map(({ name }) => name)) === JSON.stringify([
    "config", "passport", "partner_config", "payment_receipt", "vault",
    "agent", "treasury", "ip_creator", "merchant_wallet", "system_program",
  ]),
  "b2b_pay account order",
);
invariant(
  JSON.stringify(payment.args.map(({ name }) => name)) ===
    JSON.stringify(["amount_lamports", "payment_id", "order_id"]),
  "b2b_pay argument order",
);
const receipt = payment.accounts.find(({ name }) => name === "payment_receipt");
invariant(receipt?.writable, "payment receipt writable");
invariant(
  JSON.stringify(receipt?.pda?.seeds?.map(({ kind, path }) => [kind, path ?? null])) ===
    JSON.stringify([
      ["const", null], ["account", "agent"],
      ["account", "merchant_wallet"], ["arg", "payment_id"],
    ]),
  "payment receipt PDA seeds",
);
for (const name of ["set_passport_status", "set_partner_active"]) {
  invariant(idl.instructions.some((instruction) => instruction.name === name), `${name} exists`);
}
invariant(idl.accounts.some(({ name }) => name === "B2bPaymentReceipt"), "receipt account type");
invariant(idl.events.some(({ name }) => name === "B2bPaymentSettled"), "settlement event");
invariant(new Set(idl.errors.map(({ code }) => code)).size === idl.errors.length, "unique error codes");
invariant(new Set(idl.errors.map(({ name }) => name)).size === idl.errors.length, "unique error names");
invariant(idl.metadata.version === "0.6.0", "IDL release version");
invariant(manifest.idl_version === idl.metadata.version, "manifest/IDL version match");
invariant(cargo.includes('version = "0.6.0"'), "Cargo/IDL version match");
invariant(source.includes("payment_id_for(&order_id) == payment_id"), "source payment/order binding");
invariant(source.includes("init,\n        payer = agent,\n        space = B2bPaymentReceipt::LEN"), "source receipt initialization");
for (const name of ["initialize", "initialize_config", "register_partner"]) {
  invariant(idl.instructions.find((item) => item.name === name).accounts.find(({ name }) => name === "admin").writable, `${name} payer writable`);
}
for (const name of ["set_passport_status", "set_partner_active"]) {
  invariant(!idl.instructions.find((item) => item.name === name).accounts.find(({ name }) => name === "admin").writable, `${name} read-only admin`);
}

console.log("IDL validation passed");

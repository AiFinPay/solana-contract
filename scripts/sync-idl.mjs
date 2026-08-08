#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = path.join(root, "contract/src/lib.rs");
const cargoPath = path.join(root, "contract/Cargo.toml");
const idlPath = path.join(root, "idl/aifinpay_contract.json");
const source = fs.readFileSync(sourcePath, "utf8");
const cargo = fs.readFileSync(cargoPath, "utf8");
const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));

function discriminator(namespace, name) {
  return [...crypto.createHash("sha256").update(`${namespace}:${name}`).digest().subarray(0, 8)];
}

function requireSource(pattern, message) {
  if (!pattern.test(source)) throw new Error(message);
}

requireSource(/pub fn b2b_pay_with_split\s*\([\s\S]*payment_id:\s*\[u8; 32\][\s\S]*creator_fee_enabled:\s*bool/, "canonical v0.6 instruction signature missing");
requireSource(/seeds = \[b"b2b-payment", agent\.key\(\)\.as_ref\(\), payment_id\.as_ref\(\)\]/, "receipt replay PDA seed missing");
requireSource(/fn derive_payment_id\s*\(/, "deterministic payment ID derivation missing");
if (/pub fn b2b_pay\s*\(/.test(source)) throw new Error("legacy fee-inclusive b2b_pay remains active");
if (!/^version = "0\.6\.0"$/m.test(cargo)) throw new Error("contract package is not v0.6.0");

const constSeed = (value) => ({ kind: "const", value: [...Buffer.from(value)] });
const accountSeed = (value) => ({ kind: "account", path: value });
const argSeed = (value) => ({ kind: "arg", path: value });

const instruction = {
  name: "b2b_pay_with_split",
  docs: [
    "Canonical v0.6 native-SOL fee-on-top settlement.",
    "Merchant receives merchant_amount_lamports in full; protocol and optional creator fees are added on top.",
    "payment_receipt is a single-use PDA scoped to payer + deterministic payment_id.",
  ],
  discriminator: discriminator("global", "b2b_pay_with_split"),
  accounts: [
    { name: "config", pda: { seeds: [constSeed("config")] } },
    { name: "vault", pda: { seeds: [constSeed("vault")] } },
    { name: "agent", writable: true, signer: true },
    {
      name: "payment_receipt",
      writable: true,
      pda: { seeds: [constSeed("b2b-payment"), accountSeed("agent"), argSeed("payment_id")] },
    },
    { name: "treasury", writable: true },
    { name: "ip_creator", writable: true },
    { name: "merchant_wallet", writable: true },
    { name: "system_program", address: "11111111111111111111111111111111" },
  ],
  args: [
    { name: "merchant_amount_lamports", type: "u64" },
    { name: "payment_id", type: { array: ["u8", 32] } },
    { name: "order_id", type: "string" },
    { name: "creator_fee_enabled", type: "bool" },
  ],
};

const receiptType = {
  name: "B2bPaymentReceipt",
  docs: ["Single-use native-SOL payment receipt.", "Seeds: [\"b2b-payment\", payer, payment_id]"],
  type: {
    kind: "struct",
    fields: [
      { name: "payer", type: "pubkey" },
      { name: "merchant", type: "pubkey" },
      { name: "treasury", type: "pubkey" },
      { name: "creator", type: "pubkey" },
      { name: "payment_id", type: { array: ["u8", 32] } },
      { name: "merchant_amount", type: "u64" },
      { name: "treasury_fee", type: "u64" },
      { name: "creator_fee", type: "u64" },
      { name: "total_amount", type: "u64" },
      { name: "created_at", type: "i64" },
      { name: "creator_fee_enabled", type: "bool" },
      { name: "bump", type: "u8" },
    ],
  },
};

const eventType = {
  name: "B2bPaymentEvent",
  type: {
    kind: "struct",
    fields: [
      { name: "payment_id", type: { array: ["u8", 32] } },
      { name: "payer", type: "pubkey" },
      { name: "merchant", type: "pubkey" },
      { name: "treasury", type: "pubkey" },
      { name: "creator", type: "pubkey" },
      { name: "merchant_amount", type: "u64" },
      { name: "treasury_fee", type: "u64" },
      { name: "creator_fee", type: "u64" },
      { name: "total_amount", type: "u64" },
      { name: "creator_fee_enabled", type: "bool" },
      { name: "order_id", type: "string" },
    ],
  },
};

const newErrors = [
  [6020, "PaymentAmountInvalid", "B2B merchant amount must be positive and produce a protocol fee"],
  [6021, "OrderIdInvalid", "B2B order ID must contain 1..64 bytes"],
  [6022, "PaymentIdMismatch", "B2B payment ID does not match the canonical v0.6 derivation"],
  [6023, "CreatorFeeUnderflow", "B2B creator fee underflows for this merchant amount"],
  [6024, "InvalidMerchant", "B2B merchant must be a distinct system-owned wallet"],
  [6025, "InvalidCreator", "B2B creator account does not match creator_fee_enabled"],
].map(([code, name, msg]) => ({ code, name, msg }));

idl.metadata.version = "0.6.0";
idl.metadata.description = "AiFinPay Solana program v0.6 — replay-safe native fee-on-top settlement";
idl.instructions = [instruction, ...idl.instructions.filter(({ name }) => !["b2b_pay", "b2b_pay_with_split"].includes(name))];
idl.accounts = [
  { name: "B2bPaymentReceipt", discriminator: discriminator("account", "B2bPaymentReceipt") },
  ...idl.accounts.filter(({ name }) => name !== "B2bPaymentReceipt"),
];
idl.events = [
  { name: "B2bPaymentEvent", discriminator: discriminator("event", "B2bPaymentEvent") },
  ...idl.events.filter(({ name }) => name !== "B2bPaymentEvent"),
];
idl.errors = [...idl.errors.filter(({ code }) => code < 6020), ...newErrors];
idl.types = [
  receiptType,
  eventType,
  ...idl.types.filter(({ name }) => !["B2bPaymentReceipt", "B2bPaymentEvent"].includes(name)),
];

const generated = `${JSON.stringify(idl, null, 2)}\n`;
if (process.argv.includes("--write")) {
  fs.writeFileSync(idlPath, generated);
} else if (fs.readFileSync(idlPath, "utf8") !== generated) {
  throw new Error("IDL drift detected; run: node scripts/sync-idl.mjs --write");
}

// SYNTHETIC TEST KEYS ONLY. Independent Node implementation of the versioned
// HKDF-SHA-256 / AES-256-GCM wrapper construction; never use these keys in Wisp.
import { createCipheriv, hkdfSync } from 'node:crypto';

const scope = {
  origin: 'https://example.invalid',
  network: '11111111-1111-4111-8111-111111111111',
  account: '22222222-2222-4222-8222-222222222222',
};
const generation = '33333333-3333-4333-8333-333333333333';
const exportKey = Buffer.from(Array.from({length: 64}, (_, i) => i));
const vaultKey = Buffer.from(Array.from({length: 32}, (_, i) => 128 + i));
const nonce = Buffer.from(Array.from({length: 12}, (_, i) => i));
const domain = 'wisp-account-vault-wrap-v1';
const wrappingKey = hkdfSync('sha256', exportKey, 'wisp-account-vault-hkdf-v1',
  Buffer.from(JSON.stringify([domain, 1, scope])), 32);
const cipher = createCipheriv('aes-256-gcm', wrappingKey, nonce);
cipher.setAAD(Buffer.from(JSON.stringify([domain, 1, scope, generation, nonce.toString('base64')])));
const encrypted = Buffer.concat([cipher.update(vaultKey), cipher.final(), cipher.getAuthTag()]);
process.stdout.write(JSON.stringify({
  warning: 'Synthetic public test vector. Never use these keys for an account.',
  export_key_base64: exportKey.toString('base64'),
  vault_key_base64: vaultKey.toString('base64'),
  wrapper: { format: 1, scope, credential_generation: generation,
    nonce: nonce.toString('base64'), ciphertext: encrypted.toString('base64') },
}, null, 2) + '\n');

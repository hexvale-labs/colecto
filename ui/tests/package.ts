// Builds small .colectoext packages in tests: an uncompressed zip of a manifest and
// client files, which is all the Host's reader needs.
const table = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc32 = (bytes: Uint8Array) => { let c = 0xffffffff; for (const b of bytes) c = table[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };

export function zip(files: Record<string, string | Uint8Array>): Buffer {
  const local: Buffer[] = []; const central: Buffer[] = []; let offset = 0;
  for (const [name, content] of Object.entries(files)) {
    const data = Buffer.from(typeof content === 'string' ? new TextEncoder().encode(content) : content);
    const path = Buffer.from(name); const crc = crc32(data);
    const header = Buffer.alloc(30); header.writeUInt32LE(0x04034b50, 0); header.writeUInt16LE(20, 4); header.writeUInt32LE(crc, 14); header.writeUInt32LE(data.length, 18); header.writeUInt32LE(data.length, 22); header.writeUInt16LE(path.length, 26);
    const entry = Buffer.alloc(46); entry.writeUInt32LE(0x02014b50, 0); entry.writeUInt16LE(20, 4); entry.writeUInt16LE(20, 6); entry.writeUInt32LE(crc, 16); entry.writeUInt32LE(data.length, 20); entry.writeUInt32LE(data.length, 24); entry.writeUInt16LE(path.length, 28); entry.writeUInt32LE(offset, 42);
    local.push(header, path, data); central.push(entry, path); offset += 30 + path.length + data.length;
  }
  const directory = Buffer.concat(central);
  const end = Buffer.alloc(22); end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(central.length / 2, 8); end.writeUInt16LE(central.length / 2, 10); end.writeUInt32LE(directory.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, directory, end]);
}

/** A third-party package with a pane whose client part is `script`. */
export function pane(id: string, script: string, capabilities = ['account.read']) {
  return zip({
    'manifest.toml': `id = "${id}"\nname = "${id}"\ndescription = "A test extension."\nauthor = "Tests"\napi_version = 1\nclient = "client/main.js"\ncapabilities = ${JSON.stringify(capabilities)}\n\n[pane]\nicon = "extension"\n`,
    'client/main.js': script,
  });
}

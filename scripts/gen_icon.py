import zlib, struct

def make_png(filename, width, height):
    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data) & 0xffffffff)

    raw_data = bytearray()
    for y in range(height):
        raw_data.append(0)
        for x in range(width):
            r = int(30 + (x / width) * 40)
            g = int(20 + (y / height) * 60)
            b = int(70 + (x / width) * 120)
            raw_data.extend([r, g, b, 255])

    ihdr = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw_data), 9)

    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', ihdr) + chunk(b'IDAT', idat) + chunk(b'IEND', b'')
    with open(filename, 'wb') as f:
        f.write(png)

make_png("app-icon.png", 512, 512)
print("app-icon.png created")

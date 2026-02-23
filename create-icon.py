import struct, zlib, math

def create_png(width, height, pixels):
    def chunk(ctype, data):
        c = ctype + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    raw = b''
    for y in range(height):
        raw += b'\x00'
        for x in range(width):
            raw += bytes(pixels(x, y))
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw))
            + chunk(b'IEND', b''))

def icon(x, y):
    cx, cy = 512, 512
    dx, dy = abs(x - cx), abs(y - cy)
    r = math.sqrt(dx * dx + dy * dy)
    # Outer ring
    if r > 460:
        return (0, 0, 0, 0)
    if r > 440:
        return (102, 136, 204, 255)
    if r > 400:
        return (0, 0, 0, 255)
    # LCARS cross bars
    if dy < 60 and dx < 350:
        return (255, 153, 0, 255)
    if dx < 60 and dy < 350:
        return (102, 204, 204, 255)
    # Inner arc
    if r > 300 and r < 340:
        angle = math.atan2(dy, dx)
        if angle < 0.8:
            return (204, 119, 255, 255)
        if angle < 1.5:
            return (102, 136, 204, 255)
    # Center dot
    if r < 120:
        return (153, 204, 255, 255)
    if r < 125:
        return (102, 136, 204, 255)
    return (0, 8, 20, 255)

with open('app-icon.png', 'wb') as f:
    f.write(create_png(1024, 1024, icon))

print('Created app-icon.png')

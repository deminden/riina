"""Original synthetic audiovisual fixture. No downloaded media or subprocesses."""
from pathlib import Path
from io import BytesIO
import math
import struct
from PIL import Image, ImageDraw

def chunk(kind, body):
    return kind + struct.pack('<I', len(body)) + body + (b'\0' if len(body) % 2 else b'')

def group(kind, body):
    return chunk(b'LIST', kind + body)

width, height, fps, frames = 320, 180, 24, 120
avih = chunk(b'avih', struct.pack('<14I', 1000000 // fps, 300000, 0, 0x10, frames, 0, 2, 100000, width, height, 0, 0, 0, 0))
video_header = struct.pack('<4s4sIHH8I4h', b'vids', b'MJPG', 0, 0, 0, 0, 1, fps, 0, frames, 100000, 0xffffffff, 0, 0, 0, width, height)
video_format = struct.pack('<IiiHH4sIiiII', 40, width, height, 1, 24, b'MJPG', width * height * 3, 0, 0, 0, 0)
audio_header = struct.pack('<4s4sIHH8I4h', b'auds', b'\0' * 4, 0, 0, 0, 0, 4, 192000, 0, frames * 2000, 8000, 0xffffffff, 4, 0, 0, 0, 0)
audio_format = struct.pack('<HHIIHH', 1, 2, 48000, 192000, 4, 16)
headers = group(b'hdrl', avih + group(b'strl', chunk(b'strh', video_header) + chunk(b'strf', video_format)) + group(b'strl', chunk(b'strh', audio_header) + chunk(b'strf', audio_format)))
media = bytearray()
index = bytearray()
for frame in range(frames):
    image = Image.new('RGB', (width, height), '#142d46')
    draw = ImageDraw.Draw(image)
    x = 10 + int(frame / (frames - 1) * 240)
    draw.rectangle((x, 70, x + 60, 130), fill='#f9b44b')
    draw.text((12, 12), f'wiina synthetic / TOP / frame {frame:03}', fill='white')
    draw.text((12, 155), 'BOTTOM / 320 x 180 / 24 fps', fill='white')
    buffer = BytesIO()
    image.save(buffer, format='JPEG', quality=85)
    audio = bytearray()
    for sample in range(frame * 2000, (frame + 1) * 2000):
        value = int(1000 * math.sin(2 * math.pi * 440 * sample / 48000))
        audio.extend(struct.pack('<hh', value, value))
    for tag, payload in ((b'00dc', buffer.getvalue()), (b'01wb', bytes(audio))):
        index.extend(struct.pack('<4sIII', tag, 0x10, len(media) + 4, len(payload)))
        media.extend(chunk(tag, payload))
body = b'AVI ' + headers + group(b'movi', bytes(media)) + chunk(b'idx1', bytes(index))
path = Path(__file__).resolve().parents[1] / 'tests/fixtures/moving-square.avi'
path.write_bytes(b'RIFF' + struct.pack('<I', len(body)) + body)
print(path, path.stat().st_size)

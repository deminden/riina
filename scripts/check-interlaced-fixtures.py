"""Independently decode woven fields and check the bwdif fixture oracle.

Requires the development-only PyAV 16.0.1 wheel (FFmpeg 8). Nothing from this
runtime is bundled in riina. This validates fixture content and a software-filter
oracle, not the application's displayed pixels or hardware deinterlacer.
"""
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'tools/fixture-python'))
import av
import numpy as np

assert av.__version__ == '16.0.1', 'Use pinned development-only PyAV 16.0.1'


def luma(frame):
    assert frame.format.name in ('yuv422p10le', 'yuv420p')
    return frame.to_ndarray(format='gray')


def edge(row):
    bright = np.flatnonzero(row > 128)
    assert len(bright) == 12, f'Unexpected moving-bar width: {len(bright)}'
    return int(bright[0])


for name, top_first in [('interlaced-tff.mkv', True), ('interlaced-bff.mkv', False),
                        ('interlaced-mpeg2-tff.mkv', True), ('interlaced-mpeg2-bff.mkv', False)]:
    with av.open(str(root / 'tests/fixtures' / name)) as container:
        frames = list(container.decode(video=0))
    assert len(frames) == 75
    graph = av.filter.Graph()
    source = graph.add_buffer(template=frames[0])
    deinterlacer = graph.add('bwdif', 'mode=send_field:parity=auto:deint=all')
    sink = graph.add('buffersink')
    source.link_to(deinterlacer)
    deinterlacer.link_to(sink)
    graph.configure()
    output = []

    def drain():
        while True:
            try:
                output.append(graph.pull())
            except (av.error.BlockingIOError, av.error.EOFError):
                return

    for number, frame in enumerate(frames):
        assert frame.interlaced_frame, f'{name}: decoder did not flag woven fields'
        assert (frame.width, frame.height) == (96, 64)
        assert abs(float(frame.pts * frame.time_base) - number / 25) < 1e-9
        values = luma(frame)
        for row in [24, 25]:
            field = number * 2 + ((row % 2) if top_first else (1 - row % 2))
            assert edge(values[row]) == 8 + field * 2 % 64
        assert edge(values[24]) != edge(values[25]), 'Source must contain temporal comb edges'
        graph.push(frame)
        drain()
    graph.push(None)
    drain()
    assert len(output) == 150, f'{name}: expected one output per temporal field'
    for number, frame in enumerate(output):
        assert not frame.interlaced_frame
        assert abs(float(frame.pts * frame.time_base) - number / 50) < 1e-9
        values = luma(frame)
        expected = 8 + number * 2 % 64
        assert edge(values[24]) == expected, f'{name}: temporal field ordering mismatch'
        assert edge(values[25]) == expected, f'{name}: residual comb edge in software oracle'
    print(f'{name}: 75 decoded woven frames; 150 ordered progressive fields; centre comb edges removed')
print('Fixture oracle validated; riina rendered pixels remain a separate check')

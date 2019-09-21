import numpy
import numpy.linalg

# define different mixes for channels!

# stereo mix is an average
stereo_to_mono = numpy.array([[0.5, 0.5]])

# ATSC mix for 5.1
# http://www.atsc.org/wp-content/uploads/2015/03/A52-201212-17.pdf
surround_5_1_to_stereo = numpy.array(
    # L    R    C      LFE  Ls     Rs
    [[1.0, 0.0, 0.707, 0.0, 0.707, 0.0  ],
     [0.0, 1.0, 0.707, 0.0, 0.0,   0.707]]
)

# key is (new_channels, old_channels)
# only include downmixes! we can do upmixes with pseudoinverses
mixes = {
    (1, 2): stereo_to_mono,
    (2, 6): surround_5_1_to_stereo,
    (1, 6): stereo_to_mono @ surround_5_1_to_stereo,
}

# use normalize to ensure no clipping beyond -1 to 1, even in worst case
# this may change perceived loudness
def find_mix(new_channels, old_channels, normalize=False):
    if new_channels == old_channels:
        return numpy.identity(new_channels)
    try:
        mix = mixes[(min(new_channels, old_channels), max(new_channels, old_channels))]
    except KeyError:
        raise RuntimeError('cannot mix from {} channel{} to {} channel{}'.format(old_channels, 's' if old_channels != 1 else '', new_channels, 's' if new_channels != 1 else 0))
    if new_channels > old_channels:
        mix = numpy.linalg.pinv(mix)
    if normalize:
        worst_case = numpy.max(numpy.sum(numpy.abs(mix), axis=1), axis=0)
        mix /= worst_case
    return mix

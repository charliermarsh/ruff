import mmap, select, socket
# These should be fixed
try:
    pass
except EnvironmentError:
    pass

try:
    pass
except IOError:
    pass

try:
    pass
except WindowsError:
    pass

try:
    pass
except mmap.error:
    pass

try:
    pass
except select.error:
    pass

try:
    pass
except socket.error:
    pass

# Should NOT be in parentheses when replaced

try:
    pass
except (IOError,):
    pass

try:
    pass
except (EnvironmentError, IOError, OSError):
    pass

# Should be kept in parentheses (because multiple)

try:
    pass
except (IOError, KeyError, OSError):
    pass

# These should not change

from foo import error

try:
    pass
except (IOError, error):
    pass

try:
    pass
except:
    pass

try:
    pass
except AssertionError:
    pass

try:
    pass
except (mmap).error:
    pass

try:
    pass
except OSError:
    pass

try:
    pass
except (OSError, KeyError):
    pass

from .mmap import error
try:
    pass
except (IOError, error):
    pass

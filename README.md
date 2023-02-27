# Tarsier
**Shader playground powered by Rust and Vulkan**

___

Staging layouts

(undefined -> transfer dst) -> (transfer dst -> shader read only)

Staging:
from: undefined,
to: transfer dst
src stage mask: top of pipe
dst stage mask: transfer

Final:
from: transfer dst
to: shader read (only?)
src stage mask: transfer
dst stage mask: fragment shader
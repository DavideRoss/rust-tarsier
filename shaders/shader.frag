#version 450

layout (binding = 1) uniform sampler2D texSampler;

layout (location = 0) in vec2 o_uv;

layout (location = 0) out vec4 outColor;

void main() {
    vec4 color = texture(texSampler, o_uv);
    outColor = color;
}
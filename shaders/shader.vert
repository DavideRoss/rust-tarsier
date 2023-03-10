#version 450

layout (binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 projection;
} ubo;

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 uv;

layout (location = 0) out vec2 o_uv;

void main() {
    gl_Position = ubo.projection * ubo.view * ubo.model * vec4(pos, 1.0);
    o_uv = uv;
}
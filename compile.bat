@echo off
if not exist "shaders-cache" mkdir "shaders-cache"
"%VULKAN_SDK%\Bin\glslc.exe" shaders\shader.vert -o shaders-cache\vert.spv
"%VULKAN_SDK%\Bin\glslc.exe" shaders\shader.frag -o shaders-cache\frag.spv
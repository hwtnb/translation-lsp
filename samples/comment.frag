#version 450

// Retry only if the remote service
// reports a temporary failure.
const int retry_count = 3; // Use a small retry budget.

/*
 * Keep the color in linear space
 * until the final output step.
 */
#define EXPOSURE 1.0 // Keep the default exposure.

// Include the following line in this comment. \
Still part of the same comment.
layout(location = 0) out vec4 color;

void main() {
    color = vec4(vec3(EXPOSURE), 1.0);
}

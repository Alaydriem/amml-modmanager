// positions arrive in physical pixels; the constant buffer holds the target size to map them to clip space.
// Colours are premultiplied and in gamma space, blended into the UNORM target with no sRGB conversion
cbuffer Screen : register(b0) {
    float2 screenSize;
    float2 padding;
};

struct VertexIn {
    float2 pos : POSITION;
    float2 uv  : TEXCOORD0;
    float4 col : COLOR0;
};

struct PixelIn {
    float4 pos : SV_POSITION;
    float2 uv  : TEXCOORD0;
    float4 col : COLOR0;
};

PixelIn vs_main(VertexIn input) {
    PixelIn output;
    output.pos = float4(input.pos.x * 2.0 / screenSize.x - 1.0, 1.0 - input.pos.y * 2.0 / screenSize.y, 0.0, 1.0);
    output.uv  = input.uv;
    output.col = input.col;
    return output;
}

Texture2D    uiTexture : register(t0);
SamplerState uiSampler : register(s0);

float4 ps_main(PixelIn input) : SV_Target {
    return input.col * uiTexture.Sample(uiSampler, input.uv);
}

// the game picture: the alpha of a back buffer means nothing, so the picture is opaque
float4 ps_picture(PixelIn input) : SV_Target {
    return input.col * float4(uiTexture.Sample(uiSampler, input.uv).rgb, 1.0);
}

"""
Export MobileNet-v3-Small to ONNX for Realphoto's AI similarity detection.

Requirements:
    pip install torch torchvision onnx

Usage:
    python scripts/export_model.py

Output:
    src-tauri/resources/mobilenet_v3_small.onnx
"""

import torch
import torchvision
import os

def main():
    print("Loading MobileNet-v3-Small (ImageNet pretrained)...")
    model = torchvision.models.mobilenet_v3_small(weights="IMAGENET1K_V1")
    model.eval()

    # Remove the final classifier to get feature vectors (576-dim)
    model.classifier = torch.nn.Identity()

    dummy_input = torch.randn(1, 3, 224, 224)

    output_path = os.path.join(
        os.path.dirname(__file__), "..", "src-tauri", "resources", "mobilenet_v3_small.onnx"
    )
    output_path = os.path.abspath(output_path)

    print(f"Exporting to {output_path}...")
    torch.onnx.export(
        model,
        dummy_input,
        output_path,
        opset_version=13,
        input_names=["input"],
        output_names=["output"],
        dynamic_axes={"input": {0: "batch"}, "output": {0: "batch"}},
    )

    size_mb = os.path.getsize(output_path) / (1024 * 1024)
    print(f"Done! Model size: {size_mb:.1f} MB")

    # Verify
    with torch.no_grad():
        out = model(dummy_input)
    print(f"Feature vector dimension: {out.shape[1]}")


if __name__ == "__main__":
    main()

// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "ShellumiCore",
    platforms: [
        .iOS(.v16),
        .macOS(.v13)
    ],
    products: [
        .library(
            name: "ShellumiCore",
            targets: ["ShellumiCore"]
        ),
    ],
    targets: [
        // Binary target referencing the compiled XCFramework from Rust core
        .binaryTarget(
            name: "ShellumiCoreFramework",
            path: "Frameworks/ShellumiCore.xcframework"
        ),
        // Swift wrapper module exposing idiomatic Swift APIs around the C-ABI
        .target(
            name: "ShellumiCore",
            dependencies: ["ShellumiCoreFramework"],
            path: "Sources/ShellumiCore",
            publicHeadersPath: "include"
        )
    ]
)

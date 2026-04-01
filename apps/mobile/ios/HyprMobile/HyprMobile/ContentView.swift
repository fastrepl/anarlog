import SwiftUI

struct ContentView: View {
  @State private var markdownOutput = ""
  @State private var roundtripOutput = ""
  @State private var errorMessage: String?

  private let sampleJson = """
    {
      "type": "doc",
      "content": [
        {
          "type": "heading",
          "attrs": { "level": 1 },
          "content": [{ "type": "text", "text": "Hello from Rust" }]
        },
        {
          "type": "paragraph",
          "content": [
            { "type": "text", "text": "This markdown was converted from " },
            { "type": "text", "text": "TipTap JSON", "marks": [{ "type": "bold" }] },
            { "type": "text", "text": " using shared Rust code via " },
            { "type": "text", "text": "UniFFI", "marks": [{ "type": "italic" }] },
            { "type": "text", "text": "." }
          ]
        },
        {
          "type": "bulletList",
          "content": [
            {
              "type": "listItem",
              "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Shared Rust logic" }] }]
            },
            {
              "type": "listItem",
              "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "SwiftUI frontend" }] }]
            },
            {
              "type": "listItem",
              "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "UniFFI bindings" }] }]
            }
          ]
        }
      ]
    }
    """

  var body: some View {
    NavigationStack {
      ScrollView {
        VStack(alignment: .leading, spacing: 20) {
          Section {
            Text(markdownOutput)
              .font(.system(.body, design: .monospaced))
              .frame(maxWidth: .infinity, alignment: .leading)
              .padding()
              .background(Color(.systemGray6))
              .cornerRadius(8)
          } header: {
            Text("TipTap JSON → Markdown")
              .font(.headline)
          }

          Section {
            Text(roundtripOutput.prefix(200) + (roundtripOutput.count > 200 ? "..." : ""))
              .font(.system(.caption, design: .monospaced))
              .frame(maxWidth: .infinity, alignment: .leading)
              .padding()
              .background(Color(.systemGray6))
              .cornerRadius(8)
          } header: {
            Text("Markdown → TipTap JSON (roundtrip)")
              .font(.headline)
          }

          if let errorMessage {
            Text(errorMessage)
              .foregroundColor(.red)
              .font(.caption)
          }
        }
        .padding()
      }
      .navigationTitle("Hypr Mobile")
      .onAppear { convert() }
    }
  }

  private func convert() {
    do {
      let md = try tiptapJsonToMarkdown(json: sampleJson)
      markdownOutput = md

      let json = try markdownToTiptapJson(md: md)
      roundtripOutput = json
    } catch {
      errorMessage = error.localizedDescription
    }
  }
}

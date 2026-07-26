import argparse
import jiwer
import sys

if __name__ == "__main__":
    try:
        parser = argparse.ArgumentParser()
        parser.add_argument("--reference", required=True)
        parser.add_argument("--hypothesis", required=True)
        args = parser.parse_args()

        print(f"WER: {jiwer.wer(args.reference, args.hypothesis)}")
        print(f"MER: {jiwer.mer(args.reference, args.hypothesis)}")
        print(f"WIP: {jiwer.wip(args.reference, args.hypothesis)}")
        print(f"WIL: {jiwer.wil(args.reference, args.hypothesis)}")
        print(f"CER: {jiwer.cer(args.reference, args.hypothesis)}")
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

/* Generated from Rust. Run pnpm contracts:generate. */

export type Request =
  | {
      projectRoot: string;
      paths?: string[];
      keepPaths?: string[];
      productTemporary?: boolean;
      codex?: boolean;
      action: "inspect";
    }
  | {
      readView: string;
      baselineId: string;
      /**
       * @maxItems 100
       */
      selection: string[];
      action: "apply";
    }
  | {
      readView: string;
      baselineId: string;
      selection?: string[];
      action: "recheck";
    };

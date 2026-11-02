import type { KernelLayout } from './kernel-layout';
import type { KernelMember } from './kernel-members';

/** What the parts that draw along the traces (the wires and the data blips) read. */
export interface WiredProps {
  layout: KernelLayout;
  members: KernelMember[];
  /** The module being hovered or focused: its trace lights up and the others step back. */
  hot: string | null;
}

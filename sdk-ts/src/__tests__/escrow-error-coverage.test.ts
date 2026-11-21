import { EscrowErrorCode, ESCROW_ERROR_LABELS } from '../types';

describe('EscrowErrorCode coverage', () => {
  it('maps every numeric EscrowErrorCode to a label', () => {
    const enumValues = new Set<number>(
      Object.values(EscrowErrorCode).filter(
        (value): value is number => typeof value === 'number',
      ),
    );
    const labelCodes = new Set<number>(
      Object.keys(ESCROW_ERROR_LABELS).map((key) => Number(key)),
    );

    expect([...enumValues].sort((a, b) => a - b)).toEqual(
      [...labelCodes].sort((a, b) => a - b),
    );
  });
});

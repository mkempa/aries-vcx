import * as weak from 'weak-napi';

export abstract class GCWatcher {
  protected abstract _releaseFn: (handle: number) => void;
  // LibVCX handles invalid handles
  private _handleRef!: number;

  public async release(): Promise<void> {
    // we will not throw an error if released is called
    // on an invalid/already released handle
    this._releaseFn(this._handleRef);
  }

  // _clearOnExit creates a callback that will release the Rust Object
  // when the node Connection object is Garbage collected
  protected _clearOnExit(): void {
    const weakRef = weak(this);
    const release = this._releaseFn;
    const handle = this._handleRef;
    weak.addCallback(weakRef, () => {
      release(handle);
    });
  }

  // Can not use setter because of https://github.com/Microsoft/TypeScript/issues/2521
  protected _setHandle(handle: number): void {
    this._handleRef = handle;
    this._clearOnExit();
  }

  get handle(): number {
    return this._handleRef;
  }
}

# Scientific clipping

Scientific-domain clipping is distinct from GPU frustum clipping. Points are filtered or rejected according to `DomainClip` and `InvalidPointPolicy`; lines are clipped robustly in barycentric space and can yield multiple visible subpaths. GPU and CPU renderers perform ordinary view clipping only after scientific preparation.
# Coordinate system

`TetraPoint` stores A/B/C/D barycentric weights. Strict construction requires finite, non-negative values whose sum is one within `Tolerance`. Raw series arrays are explicit preparation input and are validated before use. `TetraGeometry` converts validated unit weights to and from Cartesian world coordinates while retaining fixed semantic vertex ordering.

The default regular tetrahedron is centred at the world origin. Custom vertices preserve A/B/C/D semantic order. Faces are identified by their opposite component and returned with outward winding.
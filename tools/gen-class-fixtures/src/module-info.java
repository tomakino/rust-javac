module fixtures.mod {
    exports fixtures.p;
    uses fixtures.p.Service;
    provides fixtures.p.Service with fixtures.p.ServiceImpl;
    requires java.base;
}

module sample.module {
    requires java.base;
    requires transitive java.logging;
    requires static java.sql;
    exports sample.pkg;
    exports sample.pkg2 to other.module, third.module;
    opens sample.pkg;
    uses sample.pkg.Service;
    provides sample.pkg.Service with sample.pkg.ServiceImpl;
}

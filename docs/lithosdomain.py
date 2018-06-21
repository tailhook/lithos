from sphinxcontrib.domaintools import custom_domain

def setup(app):
    app.add_domain(custom_domain('LithosOptions',
        name  = 'lithos',
        label = "Lithos Yaml Options",

        elements = dict(
            opt = dict(
                objname      = "Yaml Option",
                indextemplate = "pair: %s; Option",
            ),
            popt = dict(
                objname      = "Process Config Option",
                indextemplate = "pair: %s; Process Config Option",
            ),
            bopt = dict(
                objname      = "Bridge Setup Option",
                indextemplate = "pair: %s; Bridge Setup Option",
            ),
            volume = dict(
                objname      = "Volume Type",
                indextemplate = "pair: %s; Volume Type",
            ),
        )))
